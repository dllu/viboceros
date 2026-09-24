//! Projected crossings of exact NURBS spans with circles, arcs, and ellipses.
use super::{Conic, CurvedNurbs, SourceChoice, conic_pair, nurbs_roots, prefer_first_exact};
use crate::object_snap::SnapMetric;
use viboceros_document::ObjectId;
use viboceros_geometry::{Point3, Real};

pub(super) fn visit(
    curve: CurvedNurbs<'_>,
    conic: Conic,
    implicit: conic_pair::ImageConic,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    let Ok(sampler) = curve.curve.parameter_sampler() else {
        return;
    };
    for span in sampler.spans() {
        let score = |t: Real| {
            let image = metric.offset(span.evaluate(t).ok()?)?;
            implicit.score(image)
        };
        let derivative = |t: Real| {
            let (point, tangent) = span.evaluate_with_derivative(t).ok()?;
            let image = metric.offset(point)?;
            let screen_tangent = metric.tangent_direction(point, tangent)?;
            let gradient = implicit.gradient(image)?;
            Some(gradient[0] * screen_tangent[0] + gradient[1] * screen_tangent[1])
        };
        let tolerance = 8. * implicit.fit_error.max(Real::EPSILON);
        for root in nurbs_roots::isolate(&score, &derivative, 128, tolerance) {
            let Some(point_curve) = span.evaluate(root).ok() else {
                continue;
            };
            let Some(image) = metric.offset(point_curve) else {
                continue;
            };
            let Some(distance) = metric.captured_offset_distance(image) else {
                continue;
            };
            let Some((point_conic, error)) = conic_pair::closest_point(conic.locus, image, metric)
            else {
                continue;
            };
            if error > 1e-9 * metric.capture_radius().max(1.) {
                continue;
            }
            let prefer_curve = prefer_first_exact(
                SourceChoice {
                    hover_distance: curve.hover_distance,
                    mesh: false,
                    curve_priority: 0,
                    order: curve.order,
                    point: point_curve,
                },
                SourceChoice {
                    hover_distance: conic.hover_distance,
                    mesh: false,
                    curve_priority: conic.locus.priority(false),
                    order: conic.order,
                    point: point_conic,
                },
                metric,
            );
            let (owner, point) = if prefer_curve {
                (curve.owner, point_curve)
            } else {
                (conic.owner, point_conic)
            };
            emit(owner, point, distance);
        }
    }
}
