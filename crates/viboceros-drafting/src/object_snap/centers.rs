//! Center targets are admitted by proximity to their curve, not the empty center.
use super::SnapMetric;
use viboceros_geometry::{CurveRef, Point3, Real};

pub(super) fn visit(
    curve: CurveRef<'_>,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(Point3, Real),
) {
    match curve {
        CurveRef::Circle(circle) => candidate(
            circle.center(),
            circle.radius(),
            metric,
            |t| circle.point_at_angle(std::f64::consts::TAU * t).ok(),
            emit,
        ),
        CurveRef::Arc(arc) => candidate(
            arc.center(),
            arc.radius(),
            metric,
            |t| arc.point_at(t).ok(),
            emit,
        ),
        CurveRef::Ellipse(ellipse) => candidate(
            ellipse.center(),
            ellipse.radius_x().max(ellipse.radius_y()),
            metric,
            |t| ellipse.point_at_angle(std::f64::consts::TAU * t).ok(),
            emit,
        ),
        CurveRef::PolyCurve(curve) => {
            for segment in curve.segments() {
                visit(segment.as_ref(), metric, emit);
            }
        }
        _ => {} // General NURBS/closed-boundary center recognition is separate.
    }
}

fn candidate(
    center: Point3,
    radius: Real,
    metric: &impl SnapMetric,
    evaluate: impl Fn(Real) -> Option<Point3>,
    emit: &mut impl FnMut(Point3, Real),
) {
    if metric.offset(center).is_none() || outside_projected_bounds(center, radius, metric) {
        return;
    }
    let distance = projected_distance(|t| metric.distance(evaluate(t)?));
    if let Some(distance) = distance.filter(|&d| d <= metric.capture_radius()) {
        emit(center, distance);
    }
}

// A conic lies inside this model-space cube. For an affine/projective viewport
// with all corners in front of its clipping plane, its projected convex hull
// encloses the curve. Unprojectable/overflowing corners disable culling, not
// the candidate. Outward rounding plus a projection-scale guard avoids brittle
// boundary decisions. This is a broad phase, never the final capture predicate.
fn outside_projected_bounds(center: Point3, radius: Real, metric: &impl SnapMetric) -> bool {
    let coordinates = center.to_array();
    let lo = coordinates.map(|v| (v - radius).next_down());
    let hi = coordinates.map(|v| (v + radius).next_up());
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

/// A bounded numerical UI query over a native conic's normalized angular domain.
/// Scores are evaluated on the actual curve, never a bridging projected chord.
/// This is not a certified global solver for arbitrary projection callbacks or
/// arbitrarily narrow visible slivers at a camera-plane crossing.
fn projected_distance(score: impl Fn(Real) -> Option<Real>) -> Option<Real> {
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
