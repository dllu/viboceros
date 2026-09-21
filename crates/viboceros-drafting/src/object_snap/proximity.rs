//! Bounded camera-space curve proximity, shared by hover-derived snap targets.
use super::SnapMetric;
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
            let linear = if curve.degree() == 1 && common_sign_weights {
                span.evaluate(0.)
                    .ok()
                    .and_then(|p| metric.offset(p))
                    .zip(span.evaluate(1.).ok().and_then(|p| metric.offset(p)))
                    .and_then(|(a, b)| segment_distance(a, b))
            } else {
                None
            };
            linear.or_else(|| projected_distance(|t| metric.distance(span.evaluate(t).ok()?)))
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
    closest[0].hypot(closest[1]) > metric.capture_radius() + 64. * Real::EPSILON * scale
}

pub(super) fn line_distance(line: LineSegment, metric: &impl SnapMetric) -> Option<Real> {
    if let (Some(a), Some(b)) = (metric.offset(line.start()), metric.offset(line.end())) {
        segment_distance(a, b)
    } else {
        // Never bridge an invisible endpoint across the camera plane.
        projected_distance(|t| metric.distance(line.point_at(t).ok()?))
    }
}

/// Visible straight segments stay straight under the projection contract.
pub(super) fn segment_distance(a: [Real; 2], b: [Real; 2]) -> Option<Real> {
    // Normalize before differences/dots to avoid range loss in squares.
    let scale = a.into_iter().chain(b).map(Real::abs).fold(0., Real::max);
    if scale == 0. {
        return Some(0.);
    }
    let normalized_a = a.map(|v| v / scale);
    let normalized_b = b.map(|v| v / scale);
    let d = [
        normalized_b[0] - normalized_a[0],
        normalized_b[1] - normalized_a[1],
    ];
    let squared = d[0] * d[0] + d[1] * d[1];
    let t = if squared == 0. {
        0.
    } else {
        (-(normalized_a[0] * d[0] + normalized_a[1] * d[1]) / squared).clamp(0., 1.)
    };
    // Interpolate the original coordinates so a small perpendicular offset is
    // not lost when another axis has a vastly larger magnitude.
    let distance = ((1. - t) * a[0] + t * b[0]).hypot((1. - t) * a[1] + t * b[1]);
    distance.is_finite().then_some(distance)
}

/// Samples/refines one continuous normalized parameter interval. Every score
/// is on the actual curve, never a chord across branches. This is not a
/// certified global solver for high oscillation or arbitrarily narrow visible
/// slivers at a camera-plane crossing. NURBS callers must separate knot spans.
pub(super) fn projected_distance(score: impl Fn(Real) -> Option<Real>) -> Option<Real> {
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
