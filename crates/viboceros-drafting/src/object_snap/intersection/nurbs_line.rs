//! Projected crossings of finite NURBS spans and finite straight wires.
//! Evaluate the original curve at each root; sampling only isolates brackets.
use super::{CurvedNurbs, Segment, SourceChoice, nurbs_roots, prefer_first_exact};
use crate::object_snap::{SnapMetric, projected_line};
use viboceros_document::ObjectId;
use viboceros_geometry::{NurbsCurveSamplingSpan, Point3, Real};

const STATIONS: usize = 64;

pub(super) fn visit(
    curve: CurvedNurbs<'_>,
    segment: Segment,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    let Ok(sampler) = curve.curve.parameter_sampler() else {
        return;
    };
    for span in sampler.spans() {
        span_crossings(curve, span, segment, metric, emit);
    }
}

fn span_crossings(
    curve: CurvedNurbs<'_>,
    span: NurbsCurveSamplingSpan<'_>,
    segment: Segment,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    let direction = [
        segment.image_b[0] - segment.image_a[0],
        segment.image_b[1] - segment.image_a[1],
    ];
    let length = direction[0].hypot(direction[1]);
    if length == 0. || !length.is_finite() {
        return;
    }
    let direction = direction.map(|v| v / length);
    let score = |t: Real| {
        let image = metric.offset(span.evaluate(t).ok()?)?;
        Some(
            direction[0] * (image[1] - segment.image_a[1])
                - direction[1] * (image[0] - segment.image_a[0]),
        )
    };
    let derivative = |t: Real| {
        let (point, tangent) = span.evaluate_with_derivative(t).ok()?;
        let tangent = metric.tangent_direction(point, tangent)?;
        Some(direction[0] * tangent[1] - direction[1] * tangent[0])
    };
    let tolerance = 1e-9 * metric.capture_radius().max(1.);
    for root in nurbs_roots::isolate(&score, &derivative, STATIONS, tolerance) {
        let Some(point_curve) = span.evaluate(root).ok() else {
            continue;
        };
        let Some(image) = metric.offset(point_curve) else {
            continue;
        };
        let Some(distance) = metric.captured_offset_distance(image) else {
            continue;
        };
        let Some(point_line) = projected_line::point_at_image(segment.a, segment.b, image, metric)
        else {
            continue;
        };
        let Some(line_image) = metric.offset(point_line) else {
            continue;
        };
        if (line_image[0] - image[0]).hypot(line_image[1] - image[1])
            > 1e-8 * metric.capture_radius().max(1.)
        {
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
                hover_distance: segment.hover_distance,
                mesh: segment.mesh,
                curve_priority: 0,
                order: segment.order,
                point: point_line,
            },
            metric,
        );
        let (owner, point) = if prefer_curve {
            (curve.owner, point_curve)
        } else {
            (segment.owner, point_line)
        };
        emit(owner, point, distance);
    }
}
