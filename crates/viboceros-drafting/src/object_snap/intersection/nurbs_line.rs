//! Projected crossings of finite NURBS spans and finite straight wires.
//! Evaluate the original curve at each root; sampling only isolates brackets.
use super::{CurvedNurbs, Segment, SourceChoice, prefer_first_exact};
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
    let values: [Option<Real>; STATIONS + 1] =
        std::array::from_fn(|i| score(i as Real / STATIONS as Real));
    let tolerance = 1e-9 * metric.capture_radius().max(1.);
    if values.iter().flatten().all(|v| v.abs() <= tolerance) {
        return; // No isolated target along a projected overlap.
    }
    let mut roots = Vec::new();
    for i in 0..STATIONS {
        let a = i as Real / STATIONS as Real;
        let b = (i + 1) as Real / STATIONS as Real;
        if values[i].is_some_and(|v| v.abs() <= tolerance) {
            roots.push(a);
        }
        if let (Some(fa), Some(fb)) = (values[i], values[i + 1])
            && fa.signum() != fb.signum()
        {
            roots.push(bisect(&score, a, b, fa));
        }
        if let (Some(da), Some(db)) = (derivative(a), derivative(b))
            && da.signum() != db.signum()
        {
            let stationary = bisect(&derivative, a, b, da);
            if let Some(value) = score(stationary) {
                if value.abs() <= tolerance {
                    roots.push(stationary);
                } else {
                    if let Some(fa) = values[i]
                        && fa.signum() != value.signum()
                    {
                        roots.push(bisect(&score, a, stationary, fa));
                    }
                    if let Some(fb) = values[i + 1]
                        && value.signum() != fb.signum()
                    {
                        roots.push(bisect(&score, stationary, b, value));
                    }
                }
            }
        }
    }
    if values[STATIONS].is_some_and(|v| v.abs() <= tolerance) {
        roots.push(1.);
    }
    roots.sort_by(Real::total_cmp);
    roots.dedup_by(|a, b| (*a - *b).abs() <= 1e-10);
    for root in roots {
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

fn bisect(f: &impl Fn(Real) -> Option<Real>, mut a: Real, mut b: Real, mut fa: Real) -> Real {
    for _ in 0..72 {
        let middle = a * 0.5 + b * 0.5;
        if middle == a || middle == b {
            break;
        }
        let Some(fm) = f(middle) else {
            break;
        };
        if fm.signum() == fa.signum() {
            a = middle;
            fa = fm;
        } else {
            b = middle;
        }
    }
    a * 0.5 + b * 0.5
}
