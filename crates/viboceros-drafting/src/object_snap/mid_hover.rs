//! When Mid is the only enabled mode, discover the target from its whole segment.
use super::{ObjectSnapCache, SnapMetric, cache::CurveMidpoint, proximity};
use viboceros_document::{Geometry, ObjectId};
use viboceros_geometry::{CurveRef, CurveSegment3, Point3, Real, Tolerance};

pub(super) fn visit(
    geometry: &Geometry,
    id: ObjectId,
    tolerance: Tolerance,
    cache: &mut ObjectSnapCache,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(Point3, Real),
) {
    let features = match geometry {
        Geometry::NurbsCurve(curve) => cache.midpoints(id, std::iter::once(curve), tolerance),
        Geometry::PolyCurve(curve) => {
            for segment in curve.segments() {
                analytic(segment.as_ref(), metric, emit);
            }
            cache.midpoints(
                id,
                curve.segments().iter().filter_map(|segment| match segment {
                    CurveSegment3::NurbsCurve(curve) => Some(curve),
                    _ => None,
                }),
                tolerance,
            )
        }
        Geometry::NurbsSurface(surface) => cache.surface_midpoints(id, surface, tolerance),
        Geometry::Brep(brep) => {
            cache.midpoints(id, brep.edges().iter().map(|edge| edge.curve()), tolerance)
        }
        _ => {
            if let Some(curve) = geometry.curve_ref() {
                analytic(curve, metric, emit);
            }
            return;
        }
    };
    for feature in features {
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
                    proximity::projected_distance(|t| metric.distance(arc.point_at(t).ok()?)),
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
                    proximity::projected_distance(|t| {
                        metric.distance(circle.point_at_angle(std::f64::consts::TAU * t).ok()?)
                    }),
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
                    proximity::projected_distance(|t| {
                        metric.distance(ellipse.point_at_angle(std::f64::consts::TAU * t).ok()?)
                    }),
                    metric,
                    emit,
                );
            }
        }
        CurveRef::NurbsCurve(_) => {} // Cached separately by owning object.
        CurveRef::PolyCurve(_) => unreachable!("PolyCurve3 leaves cannot nest"),
    }
}

fn nurbs(feature: &CurveMidpoint, metric: &impl SnapMetric, emit: &mut impl FnMut(Point3, Real)) {
    let Some(point) = feature.point else { return };
    if feature
        .bounds
        .is_some_and(|b| proximity::outside_bounds(b.min().to_array(), b.max().to_array(), metric))
    {
        return;
    }
    let mut distance = None::<Real>;
    if let Ok(sampler) = feature.curve.parameter_sampler() {
        // Fractional, sided sampling avoids both native-domain rounding loss
        // and accidental interpolation across a full-multiplicity knot jump.
        for span in sampler.spans() {
            // Common-sign rational degree-one spans have a straight locus.
            // Typical B-rep edges do not need iterative parameter refinement.
            let linear = if feature.curve.degree() == 1 && feature.bounds.is_some() {
                span.evaluate(0.)
                    .ok()
                    .and_then(|p| metric.offset(p))
                    .zip(span.evaluate(1.).ok().and_then(|p| metric.offset(p)))
                    .and_then(|(a, b)| proximity::segment_distance(a, b))
            } else {
                None
            };
            if let Some(d) = linear.or_else(|| {
                proximity::projected_distance(|t| metric.distance(span.evaluate(t).ok()?))
            }) {
                distance = Some(distance.map_or(d, |old| old.min(d)));
            }
        }
    }
    candidate(point, distance, metric, emit);
}

fn candidate(
    point: Point3,
    hover: Option<Real>,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(Point3, Real),
) {
    let Some([x, y]) = metric.offset(point) else {
        return;
    };
    let direct = x.hypot(y);
    // The target is itself a point on the curve. Keep exact target hits even
    // when the bounded proximity refinement only approaches their parameter.
    let distance = hover.map_or(direct, |d| d.min(direct));
    if distance <= metric.capture_radius() {
        emit(point, distance)
    }
}

#[cfg(test)]
mod tests;
