//! Center targets are admitted by proximity to their curve, not the empty center.
use super::proximity::{outside_sphere, projected_capture_distance};
use super::{ObjectSnapCache, SnapMetric, proximity};
use viboceros_document::Object;
use viboceros_geometry::{CurveRef, Point3, Real, Tolerance};

pub(super) fn visit_nurbs(
    object: &Object,
    tolerance: Tolerance,
    cache: &mut ObjectSnapCache,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(Point3, Real),
) {
    for feature in cache.geometry_curves(object, tolerance) {
        if feature.bounds.is_some_and(|b| {
            proximity::outside_bounds(b.min().to_array(), b.max().to_array(), metric)
        }) {
            continue;
        }
        let Some(center) = feature.conic_center() else {
            continue;
        };
        if metric.offset(center).is_none() {
            continue;
        }
        // Capture the original arc, not the rest of its supporting circle.
        // A center is not on the curve and cannot use Mid's direct-hit fallback.
        if let Some(distance) =
            proximity::nurbs_distance(&feature.curve, feature.bounds.is_some(), metric)
        {
            emit(center, distance);
        }
    }
}

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
        _ => {} // NURBS recognition and polygon corners are cached separately.
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
    if let Some(distance) = projected_capture_distance(evaluate, metric) {
        emit(center, distance);
    }
}

#[cfg(test)]
mod tests;
