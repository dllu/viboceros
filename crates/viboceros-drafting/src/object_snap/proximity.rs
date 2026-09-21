//! Bounded camera-space curve proximity, shared by hover-derived snap targets.
use super::{SnapMetric, projected_line};
use viboceros_geometry::{LineSegment, NurbsCurve, Point3, Real};

/// Proximity to the original NURBS locus, shared by Mid and circular Center.
/// Sided fractional spans cannot bridge discontinuities or lose small offsets
/// when the native parameter origin is large. Callers perform bounding-box
/// rejection before this query; common-sign degree-one spans have a line locus.
pub(super) fn nurbs_distance(
    curve: &NurbsCurve,
    common_sign_weights: bool,
    metric: &impl SnapMetric,
) -> Option<Real> {
    let sampler = curve.parameter_sampler().ok()?;
    sampler
        .spans()
        .filter_map(|span| {
            if curve.degree() == 1
                && common_sign_weights
                && let (Ok(a), Ok(b)) = (span.evaluate(0.), span.evaluate(1.))
            {
                return segment_capture_distance(a, b, metric);
            }
            projected_capture_distance(|t| span.evaluate(t).ok(), metric)
        })
        .min_by(Real::total_cmp)
}

pub(super) fn outside_sphere(center: Point3, radius: Real, metric: &impl SnapMetric) -> bool {
    let coordinates = center.to_array();
    outside_bounds(
        coordinates.map(|v| v - radius),
        coordinates.map(|v| v + radius),
        metric,
    )
}

// An affine/projective viewport preserves convexity in its visible half-space.
// Any unprojectable/overflowing corner disables culling, not the candidate.
pub(super) fn outside_bounds(lo: [Real; 3], hi: [Real; 3], metric: &impl SnapMetric) -> bool {
    let lo = lo.map(Real::next_down);
    let hi = hi.map(Real::next_up);
    let mut min = [Real::INFINITY; 2];
    let mut max = [Real::NEG_INFINITY; 2];
    for bits in 0..8 {
        let xyz = std::array::from_fn(|i| if bits & (1 << i) == 0 { lo[i] } else { hi[i] });
        let Some(offset) = Point3::try_from(xyz).ok().and_then(|p| metric.offset(p)) else {
            return false;
        };
        for axis in 0..2 {
            min[axis] = min[axis].min(offset[axis]);
            max[axis] = max[axis].max(offset[axis]);
        }
    }
    let closest: [Real; 2] = std::array::from_fn(|i| {
        if min[i] > 0. {
            min[i]
        } else if max[i] < 0. {
            -max[i]
        } else {
            0.
        }
    });
    let scale = min
        .into_iter()
        .chain(max)
        .map(Real::abs)
        .fold(1., Real::max);
    closest[0].max(closest[1]) > metric.capture_radius() + 64. * Real::EPSILON * scale
}

pub(super) fn line_distance(line: LineSegment, metric: &impl SnapMetric) -> Option<Real> {
    segment_capture_distance(line.start(), line.end(), metric)
}

fn segment_capture_distance(a: Point3, b: Point3, metric: &impl SnapMetric) -> Option<Real> {
    if let Some(offset) = projected_line::closest_offset(a, b, metric) {
        return metric.captured_offset_distance(offset);
    }
    projected_capture_distance(|t| projected_line::interpolate(a, b, t), metric)
}

/// Minimize Euclidean distance first, then apply square-aperture admission to
/// that locus point. Do not clamp a missed closest point to the box boundary.
pub(super) fn projected_capture_distance(
    evaluate: impl Fn(Real) -> Option<Point3>,
    metric: &impl SnapMetric,
) -> Option<Real> {
    let mut best: Option<(Real, bool)> = None;
    projected_distance(|t| {
        let offset = metric.offset(evaluate(t)?)?;
        let distance = offset[0].hypot(offset[1]);
        if !distance.is_finite() {
            return None;
        }
        let captured = offset.iter().all(|v| v.abs() <= metric.capture_radius());
        if best.is_none_or(|(d, inside)| distance < d || (distance == d && captured && !inside)) {
            best = Some((distance, captured));
        }
        Some(distance)
    })?;
    best.filter(|(_, captured)| *captured)
        .map(|(distance, _)| distance)
}

/// Samples/refines one continuous normalized parameter interval. Every score
/// is on the actual curve, never a chord across branches. This is not a
/// certified global solver for high oscillation or arbitrarily narrow visible
/// slivers at a camera-plane crossing. NURBS callers must separate knot spans.
pub(super) fn projected_distance(mut score: impl FnMut(Real) -> Option<Real>) -> Option<Real> {
    const SAMPLES: usize = 64;
    const REFINEMENTS: usize = 8;
    const F: Real = 0.381_966_011_250_105_1;
    let values: [Real; SAMPLES + 1] =
        std::array::from_fn(|i| score(i as Real / SAMPLES as Real).unwrap_or(Real::INFINITY));
    let mut best = values.iter().copied().fold(Real::INFINITY, Real::min);
    let mut intervals: [(Real, usize); SAMPLES] =
        std::array::from_fn(|i| (values[i].min(values[i + 1]), i));
    intervals.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for &(seed, index) in intervals.iter().take(REFINEMENTS) {
        if !seed.is_finite() {
            break;
        }
        let (mut a, mut b) = (
            index as Real / SAMPLES as Real,
            (index + 1) as Real / SAMPLES as Real,
        );
        let mut left = a * (1. - F) + b * F;
        let mut right = a * F + b * (1. - F);
        let mut dl = score(left).unwrap_or(Real::INFINITY);
        let mut dr = score(right).unwrap_or(Real::INFINITY);
        for _ in 0..72 {
            best = best.min(dl).min(dr);
            if !(a < left && left < right && right < b) {
                break;
            }
            if dl <= dr {
                b = right;
                right = left;
                dr = dl;
                left = a * (1. - F) + b * F;
                dl = score(left).unwrap_or(Real::INFINITY);
            } else {
                a = left;
                left = right;
                dl = dr;
                right = a * F + b * (1. - F);
                dr = score(right).unwrap_or(Real::INFINITY);
            }
        }
    }
    best.is_finite().then_some(best)
}

#[cfg(test)]
mod tests;
