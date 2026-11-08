//! Native definitions are expectations only; sources come from authored recipes.
use super::captured::expected;
use super::*;
use viboceros_geometry::{BrepFace, BrepTrimType};

fn stations(face: &BrepFace, tolerance: Tolerance) -> Vec<Point3> {
    let s = face.surface();
    let mut samples = Vec::new();
    for i in 0..=8 {
        for j in 0..=8 {
            let u = s.parameter_at_u(i as f64 / 8.).unwrap();
            let v = s.parameter_at_v(j as f64 / 8.).unwrap();
            if face.contains_parameters(u, v, tolerance).unwrap() {
                samples.push(s.evaluate(u, v).unwrap());
            }
        }
    }
    for trim in face.loops().iter().flat_map(|l| l.trims()) {
        let c = trim.curve();
        let d = c.domain();
        for i in 0..=16 {
            let q = c
                .evaluate(
                    (*d.start() + (*d.end() - *d.start()) * i as f64 / 16.)
                        .clamp(*d.start(), *d.end()),
                )
                .unwrap();
            samples.push(
                s.evaluate(
                    q.x().clamp(*s.domain_u().start(), *s.domain_u().end()),
                    q.y().clamp(*s.domain_v().start(), *s.domain_v().end()),
                )
                .unwrap(),
            );
        }
    }
    assert!(!samples.is_empty());
    samples
}

fn distance(points: &[Point3], b: &Brep, face: usize, tolerance: Tolerance) -> f64 {
    let mut error = 0_f64;
    let s = b.faces()[face].surface();
    for &p in points {
        let (u, v) = b
            .closest_parameters_on_face(face, p, tolerance)
            .unwrap()
            .unwrap();
        let q = s
            .evaluate(
                u.clamp(*s.domain_u().start(), *s.domain_u().end()),
                v.clamp(*s.domain_v().start(), *s.domain_v().end()),
            )
            .unwrap();
        error = error.max(p.distance_to(q).unwrap());
        if error > 2e-6 {
            break;
        }
    }
    error
}
fn normal_error(a: &BrepFace, b: &BrepFace, p: Point3, t: Tolerance) -> f64 {
    let at_pole = |face: &BrepFace| {
        let s = face.surface();
        let scale = s
            .control_points()
            .iter()
            .flat_map(|c| c.point().to_array())
            .map(f64::abs)
            .fold(1_f64, f64::max);
        face.loops()
            .iter()
            .flat_map(|l| l.trims())
            .filter(|c| c.trim_type() == BrepTrimType::Singular)
            .any(|c| {
                let uv = c.curve().start_point().unwrap();
                let pole = s
                    .evaluate(
                        uv.x().clamp(*s.domain_u().start(), *s.domain_u().end()),
                        uv.y().clamp(*s.domain_v().start(), *s.domain_v().end()),
                    )
                    .unwrap();
                pole.distance_to(p).unwrap() <= 64. * f64::EPSILON * scale
            })
    };
    if at_pole(a) || at_pole(b) {
        return 0.;
    }
    let (u, v) = a.surface().closest_parameters(p, t).unwrap();
    let (x, y) = b.surface().closest_parameters(p, t).unwrap();
    match (a.surface().normal_at(u, v), b.surface().normal_at(x, y)) {
        (Ok(n), Ok(m)) => {
            let n = n
                .as_vector()
                .scaled(if a.is_reversed() { -1. } else { 1. })
                .unwrap();
            let m = m
                .as_vector()
                .scaled(if b.is_reversed() { -1. } else { 1. })
                .unwrap();
            Point3::try_from(n.to_array())
                .unwrap()
                .distance_to(Point3::try_from(m.to_array()).unwrap())
                .unwrap()
        }
        // A polar disk has an undefined parametric normal at its center,
        // while a planar disk can have a regular chart at that same point.
        // Geometry is still compared there; normals require two regular charts.
        _ => 0.,
    }
}
fn point_bounds(points: &[Point3]) -> viboceros_geometry::BoundingBox3 {
    viboceros_geometry::BoundingBox3::from_points(points.iter().copied()).unwrap()
}
fn bounds_gap(a: viboceros_geometry::BoundingBox3, b: viboceros_geometry::BoundingBox3) -> f64 {
    a.min().distance_to(b.min()).unwrap() + a.max().distance_to(b.max()).unwrap()
}
pub fn error(actual: &Brep, row: &Value, tolerance: Tolerance) -> f64 {
    let expected = expected(row, tolerance);
    assert_eq!(actual.is_solid(), expected.is_solid());
    assert_eq!(actual.faces().len(), expected.faces().len());
    assert_eq!(actual.edges().len(), expected.edges().len());
    assert_eq!(actual.vertices().len(), expected.vertices().len());
    let mut error = 0_f64;
    let actual_samples = actual
        .faces()
        .iter()
        .map(|f| stations(f, tolerance))
        .collect::<Vec<_>>();
    let actual_bounds = actual_samples
        .iter()
        .map(|p| point_bounds(p))
        .collect::<Vec<_>>();
    let mut unused = (0..actual.faces().len()).collect::<Vec<_>>();
    for (i, face) in expected.faces().iter().enumerate() {
        let points = stations(face, tolerance);
        let bounds = point_bounds(&points);
        let mut candidates = unused
            .iter()
            .enumerate()
            .map(|(slot, &j)| (slot, j))
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            bounds_gap(bounds, actual_bounds[a.1])
                .total_cmp(&bounds_gap(bounds, actual_bounds[b.1]))
        });
        let mut best = None;
        for (slot, j) in candidates {
            let alignment = normal_error(face, &actual.faces()[j], points[0], tolerance);
            if alignment > 2e-6 {
                continue;
            }
            let cost = distance(&points, actual, j, tolerance).max(distance(
                &actual_samples[j],
                &expected,
                i,
                tolerance,
            ));
            if best.is_none_or(|(_, old)| cost < old) {
                best = Some((slot, cost));
            }
            if cost <= 2e-6 {
                break;
            }
        }
        let (slot, cost) = best.expect("matching oriented native face");
        let other = &actual.faces()[unused.remove(slot)];
        assert_eq!(other.loops().len(), face.loops().len());
        let trims = |f: &BrepFace| {
            let mut kinds = f
                .loops()
                .iter()
                .flat_map(|l| l.trims())
                .map(|t| format!("{:?}", t.trim_type()))
                .collect::<Vec<_>>();
            kinds.sort();
            kinds
        };
        assert_eq!(trims(other), trims(face));
        error = error.max(cost);
        for p in points {
            error = error.max(normal_error(face, other, p, tolerance));
        }
    }
    let curve_distance = |a: &viboceros_geometry::NurbsCurve,
                          b: &viboceros_geometry::NurbsCurve| {
        (0..=16)
            .map(|i| {
                let p = a.evaluate(a.parameter_at(i as f64 / 16.).unwrap()).unwrap();
                p.distance_to(
                    b.evaluate(b.closest_parameter(p, tolerance).unwrap())
                        .unwrap(),
                )
                .unwrap()
            })
            .fold(0_f64, f64::max)
    };
    let mut unused = (0..actual.edges().len()).collect::<Vec<_>>();
    for edge in expected.edges() {
        let bounds = point_bounds(
            &edge
                .curve()
                .control_points()
                .iter()
                .map(|p| p.point())
                .collect::<Vec<_>>(),
        );
        let mut candidates = unused
            .iter()
            .enumerate()
            .map(|(slot, &j)| (slot, j))
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            let key = |j: usize| {
                bounds_gap(
                    bounds,
                    point_bounds(
                        &actual.edges()[j]
                            .curve()
                            .control_points()
                            .iter()
                            .map(|p| p.point())
                            .collect::<Vec<_>>(),
                    ),
                )
            };
            key(a.1).total_cmp(&key(b.1))
        });
        let mut best = None;
        for (slot, j) in candidates {
            let other = actual.edges()[j].curve();
            let cost = curve_distance(edge.curve(), other).max(curve_distance(other, edge.curve()));
            if best.is_none_or(|(_, old)| cost < old) {
                best = Some((slot, cost));
            }
            if cost <= 2e-6 {
                break;
            }
        }
        let (slot, cost) = best.unwrap();
        unused.remove(slot);
        error = error.max(cost);
    }
    for vertex in expected.vertices() {
        error = error.max(
            actual
                .vertices()
                .iter()
                .map(|p| p.point().distance_to(vertex.point()).unwrap())
                .fold(f64::INFINITY, f64::min),
        );
    }
    error
}

/// Compare the oriented boundary set when the two kernels partition a periodic
/// face differently. Internal seams need not correspond to an edge in both.
pub fn set_error(actual: &Brep, row: &Value, tolerance: Tolerance) -> f64 {
    let expected = expected(row, tolerance);
    assert_eq!(actual.is_solid(), expected.is_solid());
    fn direction(a: &Brep, b: &Brep, t: Tolerance) -> f64 {
        let bounds = b
            .faces()
            .iter()
            .map(|f| point_bounds(&stations(f, t)))
            .collect::<Vec<_>>();
        let mut error = 0_f64;
        for face in a.faces() {
            for p in stations(face, t) {
                let mut candidates = (0..b.faces().len()).collect::<Vec<_>>();
                let gap = |i: usize| {
                    let lo = bounds[i].min().to_array();
                    let hi = bounds[i].max().to_array();
                    let q = p.to_array();
                    p.distance_to(
                        Point3::try_from(std::array::from_fn::<_, 3, _>(|axis| {
                            q[axis].clamp(lo[axis], hi[axis])
                        }))
                        .unwrap(),
                    )
                    .unwrap()
                };
                candidates.sort_by(|&i, &j| gap(i).total_cmp(&gap(j)));
                let mut best = f64::INFINITY;
                for i in candidates {
                    let normal = normal_error(face, &b.faces()[i], p, t);
                    if normal > 2e-6 {
                        continue;
                    }
                    let difference = distance(&[p], b, i, t).max(normal);
                    best = best.min(difference);
                    if best <= 2e-6 {
                        break;
                    }
                }
                error = error.max(best);
            }
        }
        error
    }
    direction(actual, &expected, tolerance).max(direction(&expected, actual, tolerance))
}
