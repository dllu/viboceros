//! Apparent intersections of visible straight wires and circle loci.
//! A screen crossing is mapped back to each source locus before choosing the
//! source whose wire the cursor approached. Collinear overlaps have no single
//! intersection target.
use super::{ObjectSnapCache, SnapMetric, projected_line, proximity};
use std::collections::HashSet;
use viboceros_document::{Document, Geometry, LayerId, ObjectId};
use viboceros_geometry::{Circle3, CurveRef, NurbsCurve, Point3, Real};

#[derive(Clone, Copy)]
struct Segment {
    owner: ObjectId,
    order: usize,
    mesh: bool,
    a: Point3,
    b: Point3,
    image_a: [Real; 2],
    image_b: [Real; 2],
    hover_distance: Real,
}

#[derive(Clone, Copy)]
struct Circle {
    owner: ObjectId,
    order: usize,
    locus: Circle3,
    hover_distance: Real,
}

#[derive(Clone, Copy)]
struct SourceChoice {
    hover_distance: Real,
    mesh: bool,
    curved: bool,
    order: usize,
    point: Point3,
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
    let mut circles = Vec::new();
    for (order, object) in document.objects().enumerate() {
        let attributes = object.attributes();
        if !attributes.is_visible() || !visible_layers.contains(&attributes.layer_id()) {
            continue;
        }
        let owner = object.id();
        let mut add = |a, b, mesh| add_segment(&mut segments, owner, order, mesh, a, b, metric);
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
            Geometry::Circle(circle) => add_circle(&mut circles, owner, order, *circle, metric),
            Geometry::PolyCurve(polycurve) => {
                for part in polycurve.segments() {
                    add_curve(part.as_ref(), &mut add);
                    if let CurveRef::Circle(circle) = part.as_ref() {
                        add_circle(&mut circles, owner, order, *circle, metric);
                    }
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
            | Geometry::Arc(_)
            | Geometry::Ellipse(_) => {}
        }
    }
    for first in 0..segments.len() {
        for second in first + 1..segments.len() {
            let a = segments[first];
            let b = segments[second];
            // Rhino captures a polyline's own corners, but not shared
            // vertices of wires belonging to one mesh.
            if a.owner == b.owner && (a.mesh || b.mesh) {
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
            let prefer_a = prefer_first(
                SourceChoice {
                    hover_distance: a.hover_distance,
                    mesh: a.mesh,
                    curved: false,
                    order: a.order,
                    point: point_a,
                },
                SourceChoice {
                    hover_distance: b.hover_distance,
                    mesh: b.mesh,
                    curved: false,
                    order: b.order,
                    point: point_b,
                },
                metric,
            );
            let (owner, point) = if prefer_a {
                (a.owner, point_a)
            } else {
                (b.owner, point_b)
            };
            emit(owner, point, distance);
        }
    }
    for circle in circles {
        for &segment in &segments {
            circle_line_crossings(circle, segment, metric, emit);
        }
    }
}

fn add_circle(
    circles: &mut Vec<Circle>,
    owner: ObjectId,
    order: usize,
    locus: Circle3,
    metric: &impl SnapMetric,
) {
    if proximity::outside_sphere(locus.center(), locus.radius(), metric) {
        return;
    }
    let Some(hover_distance) = proximity::projected_distance(|t| {
        let p = locus.point_at_angle(std::f64::consts::TAU * t).ok()?;
        let [x, y] = metric.offset(p)?;
        Some(x.hypot(y))
    }) else {
        return;
    };
    if hover_distance > metric.capture_radius().hypot(metric.capture_radius()) {
        return;
    }
    circles.push(Circle {
        owner,
        order,
        locus,
        hover_distance,
    });
}

fn circle_line_crossings(
    circle: Circle,
    segment: Segment,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    const STATIONS: usize = 64;
    let direction = [
        segment.image_b[0] - segment.image_a[0],
        segment.image_b[1] - segment.image_a[1],
    ];
    let length = direction[0].hypot(direction[1]);
    if length == 0. || !length.is_finite() {
        return;
    }
    let score = |angle: Real| {
        let point = circle.locus.point_at_angle(angle).ok()?;
        let image = metric.offset(point)?;
        let dx = image[0] - segment.image_a[0];
        let dy = image[1] - segment.image_a[1];
        let signed = direction[0].mul_add(dy, -direction[1] * dx) / length;
        signed.is_finite().then_some(signed)
    };
    let mut roots: Vec<Real> = Vec::new();
    if metric.is_affine() {
        let Some(center) = metric.offset(circle.locus.center()) else {
            return;
        };
        let (Some(x), Some(y)) = (
            circle
                .locus
                .point_at_angle(0.)
                .ok()
                .and_then(|p| metric.offset(p)),
            circle
                .locus
                .point_at_angle(std::f64::consts::FRAC_PI_2)
                .ok()
                .and_then(|p| metric.offset(p)),
        ) else {
            return;
        };
        let signed = |p: [Real; 2]| direction[0].mul_add(p[1], -direction[1] * p[0]) / length;
        let from_start = [
            center[0] - segment.image_a[0],
            center[1] - segment.image_a[1],
        ];
        let a = signed(from_start);
        let b = signed([x[0] - center[0], x[1] - center[1]]);
        let c = signed([y[0] - center[0], y[1] - center[1]]);
        let amplitude = b.hypot(c);
        if amplitude == 0. || !amplitude.is_finite() {
            return;
        }
        let ratio = -a / amplitude;
        if !ratio.is_finite() || ratio.abs() > 1. + 64. * Real::EPSILON {
            return;
        }
        let phase = c.atan2(b);
        let opening = if (ratio.abs() - 1.).abs() <= 64. * Real::EPSILON {
            if ratio > 0. { 0. } else { std::f64::consts::PI }
        } else {
            ratio.acos()
        };
        roots.push((phase + opening).rem_euclid(std::f64::consts::TAU));
        if opening != 0. && opening != std::f64::consts::PI {
            roots.push((phase - opening).rem_euclid(std::f64::consts::TAU));
        }
    } else if let Some(fitted) = projective_circle_roots(&score) {
        roots = fitted;
    } else {
        let mut previous = score(0.);
        for i in 1..=STATIONS {
            let end = std::f64::consts::TAU * i as Real / STATIONS as Real;
            let current = score(end);
            if previous == Some(0.) {
                roots.push(std::f64::consts::TAU * (i - 1) as Real / STATIONS as Real);
            } else if let (Some(mut low_score), Some(high_score)) = (previous, current)
                && low_score.signum() != high_score.signum()
            {
                let mut low = std::f64::consts::TAU * (i - 1) as Real / STATIONS as Real;
                let mut high = end;
                for _ in 0..64 {
                    let middle = (low + high) * 0.5;
                    if middle == low || middle == high {
                        break;
                    }
                    let Some(mid_score) = score(middle) else {
                        break;
                    };
                    if mid_score.signum() == low_score.signum() {
                        low = middle;
                        low_score = mid_score;
                    } else {
                        high = middle;
                    }
                }
                roots.push((low + high) * 0.5);
            }
            previous = current;
        }
    }
    for angle in roots {
        let Some(point_circle) = circle.locus.point_at_angle(angle).ok() else {
            continue;
        };
        let Some(image) = metric.offset(point_circle) else {
            continue;
        };
        let from_start = [image[0] - segment.image_a[0], image[1] - segment.image_a[1]];
        let fraction =
            (from_start[0] * direction[0] + from_start[1] * direction[1]) / length.powi(2);
        let slack = 64. * Real::EPSILON;
        if !(-slack..=1. + slack).contains(&fraction) {
            continue;
        }
        let Some(distance) = metric.captured_offset_distance(image) else {
            continue;
        };
        let Some(point_line) = projected_line::point_at_image(segment.a, segment.b, image, metric)
        else {
            continue;
        };
        let prefer_circle = prefer_first(
            SourceChoice {
                hover_distance: circle.hover_distance,
                mesh: false,
                curved: true,
                order: circle.order,
                point: point_circle,
            },
            SourceChoice {
                hover_distance: segment.hover_distance,
                mesh: segment.mesh,
                curved: false,
                order: segment.order,
                point: point_line,
            },
            metric,
        );
        let (owner, point) = if prefer_circle {
            (circle.owner, point_circle)
        } else {
            (segment.owner, point_line)
        };
        emit(owner, point, distance);
    }
}

/// For a projective camera, the signed distance to a projected line has the
/// form `(a + b cos(t) + c sin(t)) / (1 + e cos(t) + f sin(t))` when the
/// circle center has nonzero homogeneous weight. Five visible samples recover
/// the numerator's exact roots, including a double root at tangency. Check
/// independent stations because arbitrary projected callbacks need not be
/// projective. Partially clipped circles fall back to sampled sign brackets.
fn projective_circle_roots(score: &impl Fn(Real) -> Option<Real>) -> Option<Vec<Real>> {
    const N: usize = 5;
    let mut rows = [[0.; N + 1]; N];
    for (i, row) in rows.iter_mut().enumerate() {
        let angle = std::f64::consts::TAU * i as Real / N as Real;
        let (sine, cosine) = angle.sin_cos();
        let value = score(angle)?;
        *row = [1., cosine, sine, -value * cosine, -value * sine, value];
    }
    for column in 0..N {
        let pivot =
            (column..N).max_by(|&a, &b| rows[a][column].abs().total_cmp(&rows[b][column].abs()))?;
        let scale = rows[pivot][column..N]
            .iter()
            .copied()
            .map(Real::abs)
            .fold(1., Real::max);
        if rows[pivot][column].abs() <= 128. * Real::EPSILON * scale {
            return None;
        }
        rows.swap(column, pivot);
        let divisor = rows[column][column];
        for entry in &mut rows[column][column..=N] {
            *entry /= divisor;
        }
        let pivot_row = rows[column];
        for (other, row) in rows.iter_mut().enumerate() {
            if other == column {
                continue;
            }
            let factor = row[column];
            for (entry, pivot) in row[column..=N].iter_mut().zip(&pivot_row[column..=N]) {
                *entry -= factor * pivot;
            }
        }
    }
    let [a, b, c, e, f] = rows.map(|row| row[N]);
    for i in 0..N {
        let angle = std::f64::consts::TAU * (i as Real + 0.5) / N as Real;
        let (sine, cosine) = angle.sin_cos();
        let actual = score(angle)?;
        let denominator = 1. + e * cosine + f * sine;
        if !denominator.is_finite() || denominator.abs() <= 128. * Real::EPSILON {
            return None;
        }
        let predicted = (a + b * cosine + c * sine) / denominator;
        if !predicted.is_finite() || (predicted - actual).abs() > 1e-8 * actual.abs().max(1.) {
            return None;
        }
    }
    let amplitude = b.hypot(c);
    if amplitude == 0. || !amplitude.is_finite() {
        return Some(Vec::new());
    }
    let ratio = -a / amplitude;
    if !ratio.is_finite() || ratio.abs() > 1. + 64. * Real::EPSILON {
        return Some(Vec::new());
    }
    let phase = c.atan2(b);
    let opening = if (ratio.abs() - 1.).abs() <= 64. * Real::EPSILON {
        if ratio > 0. { 0. } else { std::f64::consts::PI }
    } else {
        ratio.acos()
    };
    let mut roots = vec![(phase + opening).rem_euclid(std::f64::consts::TAU)];
    if opening != 0. && opening != std::f64::consts::PI {
        roots.push((phase - opening).rem_euclid(std::f64::consts::TAU));
    }
    Some(roots)
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
    order: usize,
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
        order,
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

fn prefer_first(a: SourceChoice, b: SourceChoice, metric: &impl SnapMetric) -> bool {
    let hover_a = metric.ownership_distance(a.hover_distance);
    let hover_b = metric.ownership_distance(b.hover_distance);
    let scale = hover_a.max(hover_b).max(1.);
    let tie = 64. * Real::EPSILON * scale;
    if (hover_a - hover_b).abs() > tie {
        return hover_a < hover_b;
    }
    let front_a = metric.frontness(a.point).unwrap_or(0.);
    let front_b = metric.frontness(b.point).unwrap_or(0.);
    let depth_tie = 64. * Real::EPSILON * front_a.abs().max(front_b.abs()).max(1.);
    if (front_a - front_b).abs() > depth_tie {
        return front_a > front_b;
    }
    if a.mesh != b.mesh {
        return !a.mesh;
    }
    if a.curved != b.curved {
        return a.curved;
    }
    a.order <= b.order
}

#[cfg(test)]
mod tests;
