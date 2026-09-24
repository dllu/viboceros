//! Apparent intersections of visible straight curve segments and mesh wires.
//! A screen crossing is mapped back to each source locus before choosing the
//! source whose wire the cursor approached. Collinear overlaps have no single
//! intersection target.
use super::{ObjectSnapCache, SnapMetric, projected_line};
use std::collections::HashSet;
use viboceros_document::{Document, Geometry, LayerId, ObjectId};
use viboceros_geometry::{CurveRef, NurbsCurve, Point3, Real};

#[derive(Clone, Copy)]
struct Segment {
    owner: ObjectId,
    mesh: bool,
    a: Point3,
    b: Point3,
    image_a: [Real; 2],
    image_b: [Real; 2],
    hover_distance: Real,
}

pub(super) fn visit(
    document: &Document,
    visible_layers: &HashSet<LayerId>,
    mesh_edges: bool,
    metric: &impl SnapMetric,
    cache: &mut ObjectSnapCache,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    let mut segments = Vec::new();
    for object in document.objects() {
        let attributes = object.attributes();
        if !attributes.is_visible() || !visible_layers.contains(&attributes.layer_id()) {
            continue;
        }
        let owner = object.id();
        let mut add = |a, b, mesh| add_segment(&mut segments, owner, mesh, a, b, metric);
        match object.geometry() {
            Geometry::Mesh(_) if mesh_edges => {
                cache
                    .meshes
                    .visit_intersection_wires(object, metric, &mut |[a, b]| {
                        add(a, b, true);
                    });
            }
            Geometry::Line(line) => add(line.start(), line.end(), false),
            Geometry::Polyline(polyline) => {
                for segment in polyline.segments() {
                    add(segment.start(), segment.end(), false);
                }
            }
            Geometry::NurbsCurve(curve) => add_linear_nurbs(curve, &mut add),
            Geometry::PolyCurve(polycurve) => {
                for part in polycurve.segments() {
                    add_curve(part.as_ref(), &mut add);
                }
            }
            Geometry::Brep(brep) => {
                for edge in brep.edges() {
                    add_linear_nurbs(edge.curve(), &mut add);
                }
            }
            Geometry::NurbsSurface(_) => {
                for boundary in cache.geometry_curves(object, document.tolerance()) {
                    add_linear_nurbs(&boundary.curve, &mut add);
                }
            }
            Geometry::Mesh(_)
            | Geometry::Point(_)
            | Geometry::PointCloud(_)
            | Geometry::Circle(_)
            | Geometry::Arc(_)
            | Geometry::Ellipse(_) => {}
        }
    }
    for first in 0..segments.len() {
        for second in first + 1..segments.len() {
            let a = segments[first];
            let b = segments[second];
            if a.owner == b.owner {
                continue;
            }
            let Some(image) = crossing(a.image_a, a.image_b, b.image_a, b.image_b) else {
                continue;
            };
            let Some(distance) = metric.captured_offset_distance(image) else {
                continue;
            };
            let (Some(point_a), Some(point_b)) = (
                projected_line::point_at_image(a.a, a.b, image, metric),
                projected_line::point_at_image(b.a, b.b, image, metric),
            ) else {
                continue;
            };
            let prefer_a = prefer_first(a, b, point_a, point_b, metric);
            let (owner, point) = if prefer_a {
                (a.owner, point_a)
            } else {
                (b.owner, point_b)
            };
            emit(owner, point, distance);
        }
    }
}

fn add_curve(curve: CurveRef<'_>, add: &mut impl FnMut(Point3, Point3, bool)) {
    match curve {
        CurveRef::Line(line) => add(line.start(), line.end(), false),
        CurveRef::Polyline(polyline) => {
            for segment in polyline.segments() {
                add(segment.start(), segment.end(), false);
            }
        }
        CurveRef::NurbsCurve(curve) => add_linear_nurbs(curve, add),
        _ => {}
    }
}

fn add_linear_nurbs(curve: &NurbsCurve, add: &mut impl FnMut(Point3, Point3, bool)) {
    if curve.degree() != 1 {
        return;
    }
    let sign = curve.control_points()[0].weight().is_sign_positive();
    if curve
        .control_points()
        .iter()
        .any(|point| point.weight().is_sign_positive() != sign)
    {
        return;
    }
    if let Ok(sampler) = curve.parameter_sampler() {
        for span in sampler.spans() {
            if let (Ok(a), Ok(b)) = (span.evaluate(0.), span.evaluate(1.)) {
                add(a, b, false);
            }
        }
    }
}

fn add_segment(
    segments: &mut Vec<Segment>,
    owner: ObjectId,
    mesh: bool,
    a: Point3,
    b: Point3,
    metric: &impl SnapMetric,
) {
    let Some(visible) = projected_line::visible_segment(a, b, metric) else {
        return;
    };
    let Some(hover_distance) = projected_line::segment_distance(visible.pa, visible.pb) else {
        return;
    };
    if hover_distance
        > metric
            .capture_radius()
            .hypot(metric.capture_radius())
            .next_up()
    {
        return;
    }
    segments.push(Segment {
        owner,
        mesh,
        a: visible.a,
        b: visible.b,
        image_a: visible.pa,
        image_b: visible.pb,
        hover_distance,
    });
}

fn crossing(a: [Real; 2], b: [Real; 2], c: [Real; 2], d: [Real; 2]) -> Option<[Real; 2]> {
    let scale = a
        .into_iter()
        .chain(b)
        .chain(c)
        .chain(d)
        .map(Real::abs)
        .fold(0., Real::max);
    if scale == 0. || !scale.is_finite() {
        return None;
    }
    let [a, b, c, d] = [a, b, c, d].map(|p| p.map(|v| v / scale));
    let u = [b[0] - a[0], b[1] - a[1]];
    let v = [d[0] - c[0], d[1] - c[1]];
    let w = [c[0] - a[0], c[1] - a[1]];
    let cross = |x: [Real; 2], y: [Real; 2]| x[0].mul_add(y[1], -x[1] * y[0]);
    let denominator = cross(u, v);
    let threshold = 32. * Real::EPSILON * u[0].hypot(u[1]) * v[0].hypot(v[1]);
    if denominator.abs() <= threshold {
        return None;
    }
    let t = cross(w, v) / denominator;
    let s = cross(w, u) / denominator;
    let slack = 32. * Real::EPSILON;
    if !(-slack..=1. + slack).contains(&t) || !(-slack..=1. + slack).contains(&s) {
        return None;
    }
    let point = [
        scale * u[0].mul_add(t.clamp(0., 1.), a[0]),
        scale * u[1].mul_add(t.clamp(0., 1.), a[1]),
    ];
    point.iter().all(|v| v.is_finite()).then_some(point)
}

fn prefer_first(
    a: Segment,
    b: Segment,
    point_a: Point3,
    point_b: Point3,
    metric: &impl SnapMetric,
) -> bool {
    let scale = a.hover_distance.max(b.hover_distance).max(1.);
    let tie = 64. * Real::EPSILON * scale;
    if (a.hover_distance - b.hover_distance).abs() > tie {
        return a.hover_distance < b.hover_distance;
    }
    let front_a = metric.frontness(point_a).unwrap_or(0.);
    let front_b = metric.frontness(point_b).unwrap_or(0.);
    if front_a != front_b {
        return front_a > front_b;
    }
    if a.mesh != b.mesh {
        return !a.mesh;
    }
    true
}

#[cfg(test)]
mod tests;
