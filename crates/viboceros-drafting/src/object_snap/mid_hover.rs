//! When Mid is the only enabled mode, discover the target from its whole segment.
use super::{ObjectSnapCache, SnapMetric, cache::CurveFeatures, proximity};
use viboceros_document::Object;
use viboceros_geometry::{CurveRef, Point3, Real, Tolerance};

pub(super) fn visit(
    object: &Object,
    tolerance: Tolerance,
    cache: &mut ObjectSnapCache,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(Point3, Real),
) {
    if let Some(curve) = object.geometry().curve_ref() {
        if let CurveRef::PolyCurve(curve) = curve {
            for segment in curve.segments() {
                analytic(segment.as_ref(), metric, emit);
            }
        } else {
            analytic(curve, metric, emit);
        }
    }
    for feature in cache.geometry_curves(object, tolerance) {
        nurbs(feature, metric, emit);
    }
}

fn analytic(curve: CurveRef<'_>, metric: &impl SnapMetric, emit: &mut impl FnMut(Point3, Real)) {
    match curve {
        CurveRef::Line(line) => {
            if let Ok(point) = line.point_at(0.5) {
                candidate(point, proximity::line_distance(*line, metric), metric, emit);
            }
        }
        CurveRef::Polyline(polyline) => {
            for line in polyline.segments() {
                analytic(CurveRef::Line(&line), metric, emit);
            }
        }
        CurveRef::Arc(arc) => {
            if !proximity::outside_sphere(arc.center(), arc.radius(), metric)
                && let Ok(point) = arc.point_at(0.5)
            {
                candidate(
                    point,
                    proximity::projected_capture_distance(|t| arc.point_at(t).ok(), metric),
                    metric,
                    emit,
                );
            }
        }
        CurveRef::Circle(circle) => {
            if !proximity::outside_sphere(circle.center(), circle.radius(), metric)
                && let Ok(point) = circle.point_at_angle(std::f64::consts::PI)
            {
                candidate(
                    point,
                    proximity::projected_capture_distance(
                        |t| circle.point_at_angle(std::f64::consts::TAU * t).ok(),
                        metric,
                    ),
                    metric,
                    emit,
                );
            }
        }
        CurveRef::Ellipse(ellipse) => {
            if !proximity::outside_sphere(
                ellipse.center(),
                ellipse.radius_x().max(ellipse.radius_y()),
                metric,
            ) && let Ok(point) = ellipse.point_at_angle(std::f64::consts::PI)
            {
                candidate(
                    point,
                    proximity::projected_capture_distance(
                        |t| ellipse.point_at_angle(std::f64::consts::TAU * t).ok(),
                        metric,
                    ),
                    metric,
                    emit,
                );
            }
        }
        CurveRef::NurbsCurve(_) => {} // Cached separately by owning object.
        CurveRef::PolyCurve(_) => unreachable!("PolyCurve3 leaves cannot nest"),
    }
}

fn nurbs(feature: &CurveFeatures, metric: &impl SnapMetric, emit: &mut impl FnMut(Point3, Real)) {
    if feature
        .bounds
        .is_some_and(|b| proximity::outside_bounds(b.min().to_array(), b.max().to_array(), metric))
    {
        return;
    }
    // A common-sign control hull bounds both the curve and its Mid target.
    // Reject distant sources before cold arc-length integration, not afterward.
    let Some(point) = feature.midpoint() else {
        return;
    };
    let distance = proximity::nurbs_distance(&feature.curve, feature.bounds.is_some(), metric);
    candidate(point, distance, metric, emit);
}

fn candidate(
    point: Point3,
    hover: Option<Real>,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(Point3, Real),
) {
    let Some(offset) = metric.offset(point) else {
        return;
    };
    let direct = metric.captured_offset_distance(offset);
    // The target is itself a point on the curve. Keep exact target hits even
    // when the bounded proximity refinement only approaches their parameter.
    if let Some(distance) = hover.into_iter().chain(direct).min_by(Real::total_cmp) {
        emit(point, distance)
    }
}

#[cfg(test)]
mod tests;
