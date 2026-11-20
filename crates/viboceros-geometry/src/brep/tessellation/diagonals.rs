//! Repair lifted triangles that face against a smooth display chart.
mod cavity;
mod fan;
mod vertex_fit;

pub(super) use fan::repair_remaining;

use super::*;
use std::collections::HashMap;

type Triangle = [u32; 3];
type Edge = [u32; 2];
const NO_FACE: usize = usize::MAX;

fn edge(a: u32, b: u32) -> Edge {
    [a.min(b), a.max(b)]
}

fn wound_side([a, b, c]: [Point2; 3]) -> Real {
    let area = signed_area([a, b, c]);
    if area == 0. { 0. } else { area.signum() }
}

fn signed_area([a, b, c]: [Point2; 3]) -> Real {
    let positive = (b.x() - a.x()) * (c.y() - a.y());
    let negative = (b.y() - a.y()) * (c.x() - a.x());
    let determinant = positive - negative;
    // Leave roundoff-scale and collinear cases to the original triangulator.
    let roundoff = 8. * Real::EPSILON * (positive.abs() + negative.abs());
    if determinant.is_finite() && determinant.abs() > roundoff.max(Real::MIN_POSITIVE) {
        determinant
    } else {
        0.
    }
}

#[derive(Clone, Copy)]
struct SurfaceSample {
    uv: Point2,
    center: Point3,
    surface_point: Point3,
    normal: Vector3,
    alignment: Real,
    plane_distance: Real,
    twice_area: Real,
}

fn sample(
    surface: &NurbsSurface,
    points: &[Point3],
    parameters: &[Point2],
    triangle: Triangle,
) -> Option<SurfaceSample> {
    let [a, b, c] = triangle.map(|i| parameters[i as usize]);
    let winding = wound_side([a, b, c]);
    if winding == 0. {
        return None;
    }
    sample_in_winding(surface, points, parameters, triangle, winding)
}

fn sample_in_winding(
    surface: &NurbsSurface,
    points: &[Point3],
    parameters: &[Point2],
    triangle: Triangle,
    winding: Real,
) -> Option<SurfaceSample> {
    let [p, q, r] = triangle.map(|i| points[i as usize]);
    let [a, b, c] = triangle.map(|i| parameters[i as usize]);
    let geometric_normal = p.vector_to(q).ok()?.cross(p.vector_to(r).ok()?).ok()?;
    let model_normal = geometric_normal.normalized_nonzero().ok()?.as_vector();
    let uv = Point2::try_new(
        a.x() + (b.x() - a.x()) / 3. + (c.x() - a.x()) / 3.,
        a.y() + (b.y() - a.y()) / 3. + (c.y() - a.y()) / 3.,
    )
    .ok()?;
    let (surface_point, du, dv) = surface.evaluate_with_derivatives(uv.x(), uv.y()).ok()?;
    let surface_normal = du.cross(dv).ok()?.normalized_nonzero().ok()?;
    let center = Point3::try_new(
        p.x() + (q.x() - p.x()) / 3. + (r.x() - p.x()) / 3.,
        p.y() + (q.y() - p.y()) / 3. + (r.y() - p.y()) / 3.,
        p.z() + (q.z() - p.z()) / 3. + (r.z() - p.z()) / 3.,
    )
    .ok()?;
    Some(SurfaceSample {
        uv,
        center,
        surface_point,
        normal: model_normal,
        alignment: winding * model_normal.dot(surface_normal.as_vector()).ok()?,
        plane_distance: p
            .vector_to(surface_point)
            .ok()?
            .dot(model_normal)
            .ok()?
            .abs(),
        twice_area: geometric_normal.length().ok()?,
    })
}

fn chord_deviation(
    surface: &NurbsSurface,
    points: &[Point3],
    parameters: &[Point2],
    indices: Edge,
) -> Option<Real> {
    let [a, b] = indices.map(|i| parameters[i as usize]);
    let [left, right] = indices.map(|i| points[i as usize]);
    let mid = surface
        .evaluate(a.x().midpoint(b.x()), a.y().midpoint(b.y()))
        .ok()?;
    chords::distance_to_chord(mid, left, right).ok()
}

/// Find a nearby surface witness for the triangle's center. Replacement
/// witnesses are constrained to the affected triangles' parameter region.
fn center_distance(
    surface: &NurbsSurface,
    sample: &SurfaceSample,
    contains: impl Fn(Point2) -> bool,
) -> Option<Real> {
    center_witness(surface, sample, contains).map(|(_, distance)| distance)
}

fn center_witness(
    surface: &NurbsSurface,
    sample: &SurfaceSample,
    contains: impl Fn(Point2) -> bool,
) -> Option<(Point2, Real)> {
    let (u, v) = (surface.domain_u(), surface.domain_v());
    let mut uv = sample.uv;
    let mut distance = sample.center.distance_to(sample.surface_point).ok()?;
    for _ in 0..12 {
        let (point, du, dv) = surface.evaluate_with_derivatives(uv.x(), uv.y()).ok()?;
        let residual = point.vector_to(sample.center).ok()?;
        let (uu, uv_dot, vv) = (du.dot(du).ok()?, du.dot(dv).ok()?, dv.dot(dv).ok()?);
        let determinant = uu.mul_add(vv, -uv_dot * uv_dot);
        if !(determinant.is_finite() && determinant > Real::EPSILON * uu * vv) {
            break;
        }
        let (du_r, dv_r) = (du.dot(residual).ok()?, dv.dot(residual).ok()?);
        let step_u = (vv * du_r - uv_dot * dv_r) / determinant;
        let step_v = (uu * dv_r - uv_dot * du_r) / determinant;
        let mut improved = None;
        for backtrack in 0..8 {
            let step = 1. / Real::from(1 << backtrack);
            let candidate = Point2::try_new(
                (uv.x() + step * step_u).clamp(*u.start(), *u.end()),
                (uv.y() + step * step_v).clamp(*v.start(), *v.end()),
            )
            .ok()?;
            if !contains(candidate) {
                continue;
            }
            let point = surface.evaluate(candidate.x(), candidate.y()).ok()?;
            let next = sample.center.distance_to(point).ok()?;
            if next < distance {
                improved = Some((candidate, next));
                break;
            }
        }
        let Some((candidate, next)) = improved else {
            break;
        };
        uv = candidate;
        distance = next;
    }
    Some((uv, distance))
}

struct Flip {
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    neighbor: usize,
    deviation: Real,
}

impl Flip {
    fn triangles(&self) -> [Triangle; 2] {
        let Self { a, b, c, d, .. } = *self;
        [[a, d, c], [d, b, c]]
    }
}

pub(super) fn repair(
    surface: &NurbsSurface,
    parameters: &mut [Point2],
    points: &mut [Point3],
    triangles: &mut [Triangle],
    height: Real,
) {
    let sample = |triangle| sample(surface, points, parameters, triangle);
    let inverted: Vec<_> = triangles
        .iter()
        .enumerate()
        .filter_map(|(i, &face)| sample(face).filter(|s| s.alignment < 0.).map(|_| i))
        .collect();
    if inverted.is_empty() {
        return;
    }

    let mut adjacent = HashMap::with_capacity(triangles.len());
    for (i, &[a, b, c]) in triangles.iter().enumerate() {
        for key in [edge(a, b), edge(b, c), edge(c, a)] {
            let pair = adjacent.entry(key).or_insert([NO_FACE; 2]);
            if pair[1] != NO_FACE {
                return;
            }
            if pair[0] == NO_FACE {
                pair[0] = i;
            } else {
                pair[1] = i;
            }
        }
    }
    for &i in &inverted {
        let triangle = triangles[i];
        let Some(old) = sample(triangle).filter(|s| s.alignment < 0.) else {
            continue;
        };
        let mut best: Option<Flip> = None;
        for j in 0..3 {
            let [a, b, c] = [triangle[j], triangle[(j + 1) % 3], triangle[(j + 2) % 3]];
            let [first, second] = adjacent[&edge(a, b)];
            let neighbor = if first == i { second } else { first };
            if neighbor == NO_FACE {
                continue;
            }
            let other = triangles[neighbor];
            let Some(k) = (0..3).find(|&k| other[k] == b && other[(k + 1) % 3] == a) else {
                continue;
            };
            let d = other[(k + 2) % 3];
            if c == d || adjacent.contains_key(&edge(c, d)) {
                continue;
            }
            let flip = Flip {
                a,
                b,
                c,
                d,
                neighbor,
                deviation: 0.,
            };
            let boundary = [a, d, b, c].map(|i| parameters[i as usize]);
            let [pa, pd, pb, pc] = boundary;
            let winding = wound_side([pa, pb, pc]);
            // All four turns must agree, so changing diagonals preserves
            // the UV region and all the boundary edges of the pair.
            if winding == 0.
                || [
                    wound_side([pb, pa, pd]),
                    wound_side([pa, pd, pc]),
                    wound_side([pd, pb, pc]),
                ]
                .into_iter()
                .any(|s| s != winding)
            {
                continue;
            }
            let Some(deviation) = chord_deviation(surface, points, parameters, [c, d]) else {
                continue;
            };
            let Some(old_deviation) = chord_deviation(surface, points, parameters, [a, b]) else {
                continue;
            };
            if deviation > old_deviation.min(height).max(height / 8.)
                || best
                    .as_ref()
                    .is_some_and(|best| deviation >= best.deviation)
            {
                continue;
            }
            let [left, right] = flip.triangles();
            if [left, right].into_iter().any(|triangle| {
                !crate::nurbs_surface::tessellation_triangle_is_nondegenerate(
                    points,
                    triangle,
                    Tolerance::MESH_VALIDATION,
                )
                .unwrap_or(false)
            }) {
                continue;
            }
            let [Some(new_left), Some(new_right)] = [sample(left), sample(right)] else {
                continue;
            };
            let Some(old_other) = sample(other) else {
                continue;
            };
            if new_left.alignment <= 0.1
                || new_right.alignment <= 0.1
                || new_left.plane_distance.max(new_right.plane_distance)
                    > old
                        .plane_distance
                        .max(old_other.plane_distance)
                        .max(height / 12.)
            {
                continue;
            }
            // The old center can have a much closer witness beyond the quad.
            // Include it in the comparison to avoid inflating the allowance.
            let previous_distance =
                |sample| center_distance(surface, sample, |_| true).unwrap_or(Real::INFINITY);
            let prior = previous_distance(&old).max(previous_distance(&old_other));
            let inside_quad =
                |p| (0..4).all(|i| wound_side([boundary[i], boundary[(i + 1) % 4], p]) == winding);
            let new_distance =
                |sample| center_distance(surface, sample, inside_quad).unwrap_or(Real::INFINITY);
            let updated = new_distance(&new_left).max(new_distance(&new_right));
            if !prior.is_finite() || updated > prior.max(height / 24.) {
                continue;
            }
            best = Some(Flip { deviation, ..flip });
        }
        if let Some(flip) = best {
            let [left, right] = flip.triangles();
            let Flip {
                a,
                b,
                c,
                d,
                neighbor,
                ..
            } = flip;
            adjacent.remove(&edge(a, b));
            adjacent.insert(edge(c, d), [i, neighbor]);
            for (key, old, new) in [(edge(a, d), neighbor, i), (edge(b, c), i, neighbor)] {
                let pair = adjacent
                    .get_mut(&key)
                    .expect("outer edge belongs to the pair");
                let slot = usize::from(pair[0] != old);
                pair[slot] = new;
            }
            triangles[i] = left;
            triangles[neighbor] = right;
        }
    }
    cavity::repair(
        surface,
        parameters,
        points,
        triangles,
        height,
        &inverted,
        &mut adjacent,
    );
    vertex_fit::relocate(
        surface, parameters, points, triangles, height, &inverted, &adjacent,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // A regular shear, S(u,v) = (u, v + 2u², 0).
    fn shear() -> NurbsSurface {
        let net = [0., 1.]
            .into_iter()
            .flat_map(|v| {
                [(0., v), (0.5, v), (1., v + 2.)].map(|(x, y)| Point3::try_new(x, y, 0.).unwrap())
            })
            .collect();
        NurbsSurface::try_new(
            2,
            1,
            3,
            2,
            net,
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    }

    #[test]
    fn replaces_the_inverted_lift_of_a_triangle_in_a_convex_parameter_quad() {
        let surface = shear();
        let mut parameters = [(0.2, 0.4), (0.95, 0.35), (0.6, 0.55), (0.65, 0.2)]
            .map(|(u, v)| Point2::try_new(u, v).unwrap());
        let mut points = parameters.map(|uv| surface.evaluate(uv.x(), uv.y()).unwrap());
        let mut faces = [[0, 1, 2], [1, 0, 3]];
        assert!(
            sample(&surface, &points, &parameters, faces[0])
                .unwrap()
                .alignment
                < 0.
        );

        repair(&surface, &mut parameters, &mut points, &mut faces, 3.);

        assert_eq!(faces, [[0, 3, 2], [3, 1, 2]]);
        assert!(faces.iter().all(|&face| {
            sample(&surface, &points, &parameters, face)
                .unwrap()
                .alignment
                > 0.
        }));
    }

    #[test]
    fn retriangulates_three_faces_while_preserving_the_five_boundary_edges() {
        let surface = shear();
        let parameters = [
            (0.26, 0.45),
            (0.63, 0.25),
            (0.81, 0.8),
            (0.65, 0.8),
            (0.36, 0.59),
        ]
        .map(|(u, v)| Point2::try_new(u, v).unwrap());
        let points = parameters.map(|uv| surface.evaluate(uv.x(), uv.y()).unwrap());
        let mut faces = [[0, 1, 2], [0, 2, 3], [0, 3, 4]];
        let alignment = |face| {
            sample(&surface, &points, &parameters, face)
                .unwrap()
                .alignment
        };
        assert!(alignment(faces[1]) < 0. && alignment(faces[2]) < 0.);
        let mut adjacent = HashMap::from([
            ([0, 1], [0, NO_FACE]),
            ([1, 2], [0, NO_FACE]),
            ([2, 3], [1, NO_FACE]),
            ([3, 4], [2, NO_FACE]),
            ([0, 4], [2, NO_FACE]),
            ([0, 2], [0, 1]),
            ([0, 3], [1, 2]),
        ]);
        cavity::repair(
            &surface,
            &parameters,
            &points,
            &mut faces,
            3.,
            &[1, 2],
            &mut adjacent,
        );

        assert!(faces.into_iter().all(|face| alignment(face) > 0.));
        assert_eq!(adjacent.len(), 7);
        assert!(!adjacent.contains_key(&[0, 2]) && !adjacent.contains_key(&[0, 3]));
        for a in 0..5 {
            let b = (a + 1) % 5;
            assert_eq!(
                faces
                    .iter()
                    .filter(|face| face.contains(&a) && face.contains(&b))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn repairs_a_sheared_hexagon_when_no_smaller_partition_can_fix_it() {
        let surface = shear();
        let mut parameters = [
            (0.09, 0.28),
            (0.08, 0.25),
            (0.77, 0.65),
            (0.80, 0.84),
            (0.63, 0.87),
            (0.33, 0.82),
        ]
        .map(|(u, v)| Point2::try_new(u, v).unwrap());
        let mut points = parameters.map(|uv| surface.evaluate(uv.x(), uv.y()).unwrap());
        let mut faces = [[1, 2, 3], [0, 1, 3], [3, 4, 5], [0, 3, 5]];
        assert!(faces.iter().any(|&face| {
            sample(&surface, &points, &parameters, face).is_some_and(|s| s.alignment < 0.)
        }));

        repair(&surface, &mut parameters, &mut points, &mut faces, 3.);

        assert!(faces.iter().all(|&face| {
            sample(&surface, &points, &parameters, face).is_some_and(|s| s.alignment > 0.)
        }));
        for a in 0..6 {
            let b = (a + 1) % 6;
            assert_eq!(
                faces
                    .iter()
                    .filter(|face| face.contains(&a) && face.contains(&b))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn relocates_an_interior_vertex_on_a_sheared_chart_with_fixed_boundary() {
        let surface = shear();
        let mut uv = [(0.1, 0.1), (0.9, 0.1), (0.9, 0.95), (0.1, 0.95), (0.5, 0.2)]
            .map(|(u, v)| Point2::try_new(u, v).unwrap());
        let mut positions = uv.map(|p| surface.evaluate(p.x(), p.y()).unwrap());
        let triangles = [[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]];
        let adjacent = HashMap::from([
            ([0, 1], [0, NO_FACE]),
            ([1, 2], [1, NO_FACE]),
            ([2, 3], [2, NO_FACE]),
            ([0, 3], [3, NO_FACE]),
            ([0, 4], [3, 0]),
            ([1, 4], [0, 1]),
            ([2, 4], [1, 2]),
            ([3, 4], [2, 3]),
        ]);
        assert!(
            sample(&surface, &positions, &uv, triangles[0])
                .unwrap()
                .alignment
                < 0.
        );
        let old_uv = uv;
        let old_positions = positions;
        vertex_fit::relocate(
            &surface,
            &mut uv,
            &mut positions,
            &triangles,
            3.,
            &[0],
            &adjacent,
        );
        assert_ne!(uv[4], old_uv[4]);
        assert_eq!(uv[..4], old_uv[..4]);
        assert_eq!(positions[..4], old_positions[..4]);
        assert_eq!(
            positions[4],
            surface.evaluate(uv[4].x(), uv[4].y()).unwrap()
        );
        assert!(triangles.iter().all(|&face| {
            sample(&surface, &positions, &uv, face).is_some_and(|s| s.alignment > 0.)
        }));
    }

    #[test]
    fn inserts_a_surface_vertex_without_changing_the_cavity_perimeter() {
        let surface = shear();
        let mut uv = [(0.2, 0.4), (0.65, 0.2), (0.95, 0.35), (0.6, 0.55)]
            .map(|(u, v)| Point2::try_new(u, v).unwrap())
            .to_vec();
        let mut points = uv
            .iter()
            .map(|site| surface.evaluate(site.x(), site.y()).unwrap())
            .collect::<Vec<_>>();
        let mut faces = vec![[0, 1, 2], [0, 2, 3]];
        assert!(sample(&surface, &points, &uv, faces[1]).is_some_and(|s| s.alignment < 0.));
        let boundary = points.clone();

        repair_remaining(&surface, &mut uv, &mut points, &mut faces, 3., 5);

        assert_eq!(points.len(), 5);
        assert_eq!(points[..4], boundary);
        assert_eq!(faces.len(), 4);
        assert_eq!(points[4], surface.evaluate(uv[4].x(), uv[4].y()).unwrap());
        for i in 0..4 {
            assert!(faces.contains(&[i, (i + 1) % 4, 4]));
        }
        assert!(faces.iter().all(|&face| {
            sample(&surface, &points, &uv, face).is_some_and(|s| s.alignment > 0.)
        }));
    }
}
