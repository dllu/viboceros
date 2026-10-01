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
        if metric.camera_depth(center) == Some(0.) && feature.curve.is_closed().ok() == Some(true) {
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
        CurveRef::Circle(circle) if metric.camera_depth(circle.center()) != Some(0.) => candidate(
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
        CurveRef::Ellipse(ellipse) if metric.camera_depth(ellipse.center()) != Some(0.) => {
            candidate(
                ellipse.center(),
                ellipse.radius_x().max(ellipse.radius_y()),
                metric,
                |t| ellipse.point_at_angle(std::f64::consts::TAU * t).ok(),
                emit,
            )
        }
        CurveRef::PolyCurve(curve) => {
            for segment in curve.segments() {
                visit(segment.as_ref(), metric, emit);
            }
        }
        _ => {} // NURBS recognition and polygon corners are cached separately.
    }
}

/// Four clamped quadratic conic spans expose opposite boundary control points.
/// Coincident diagonal midpoints preserve their center when a conic fit differs
/// by a few input ULPs. Recognition has already checked the complete curve;
/// this does not admit a new locus or depend on a camera or document tolerance.
pub(super) fn stable_closed_conic_center(
    curve: &viboceros_geometry::NurbsCurve,
    fitted: Point3,
) -> Point3 {
    let controls = curve.control_points();
    let knots = curve.knots();
    if curve.degree() != 2
        || controls.len() != 9
        || knots.len() != 12
        || controls[0].point() != controls[8].point()
        || knots[0] != knots[1]
        || knots[1] != knots[2]
        || knots[9] != knots[10]
        || knots[10] != knots[11]
        || (3..=7).step_by(2).any(|i| knots[i] != knots[i + 1])
    {
        return fitted;
    }
    let Ok(center) = controls[0].point().midpoint(controls[4].point()) else {
        return fitted;
    };
    if controls[2].point().midpoint(controls[6].point()).ok() != Some(center) {
        return fitted;
    }
    for (i, (proposed, original)) in center
        .to_array()
        .into_iter()
        .zip(fitted.to_array())
        .enumerate()
    {
        let scale = controls
            .iter()
            .map(|point| point.point().to_array()[i].abs())
            .fold(0., Real::max);
        let ulp = scale.next_up() - scale;
        if !ulp.is_finite() || (proposed - original).abs() > 4. * ulp {
            return fitted;
        }
    }
    center
}

fn candidate(
    center: Point3,
    radius: Real,
    metric: &impl SnapMetric,
    evaluate: impl Fn(Real) -> Option<Point3>,
    emit: &mut impl FnMut(Point3, Real),
) {
    if outside_sphere(center, radius, metric) {
        return;
    }
    if let Some(distance) = projected_capture_distance(evaluate, metric) {
        emit(center, distance);
    }
}

#[cfg(test)]
mod tests;
