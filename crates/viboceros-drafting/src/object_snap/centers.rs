//! Center targets are admitted by proximity to their curve, not the empty center.
use super::SnapMetric;
use super::proximity::{outside_sphere, projected_distance};
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
    if metric.offset(center).is_none() || outside_sphere(center, radius, metric) {
        return;
    }
    let distance = projected_distance(|t| metric.distance(evaluate(t)?));
    if let Some(distance) = distance.filter(|&d| d <= metric.capture_radius()) {
        emit(center, distance);
    }
}

#[cfg(test)]
mod tests;
