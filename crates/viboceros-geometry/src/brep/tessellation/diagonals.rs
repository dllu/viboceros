//! Repair lifted triangles that face against a smooth display chart.
use super::*;
use std::collections::HashMap;

type Triangle = [u32; 3];
type Edge = [u32; 2];
const NO_FACE: usize = usize::MAX;

fn edge(a: u32, b: u32) -> Edge {
    [a.min(b), a.max(b)]
}

fn wound_side([a, b, c]: [Point2; 3]) -> Real {
    let positive = (b.x() - a.x()) * (c.y() - a.y());
    let negative = (b.y() - a.y()) * (c.x() - a.x());
    let determinant = positive - negative;
    // Leave roundoff-scale and collinear cases to the original triangulator.
    let roundoff = 8. * Real::EPSILON * (positive.abs() + negative.abs());
    if determinant.is_finite() && determinant.abs() > roundoff.max(Real::MIN_POSITIVE) {
        determinant.signum()
    } else {
        0.
    }
}

struct SurfaceSample {
    uv: Point2,
    center: Point3,
    surface_point: Point3,
    alignment: Real,
    plane_distance: Real,
}

fn sample(
    surface: &NurbsSurface,
    points: &[Point3],
    parameters: &[Point2],
    triangle: Triangle,
) -> Option<SurfaceSample> {
    let [p, q, r] = triangle.map(|i| points[i as usize]);
    let [a, b, c] = triangle.map(|i| parameters[i as usize]);
    let winding = wound_side([a, b, c]);
    if winding == 0. {
        return None;
    }
    let model_normal = p
        .vector_to(q)
        .ok()?
        .cross(p.vector_to(r).ok()?)
        .ok()?
        .normalized_nonzero()
        .ok()?
        .as_vector();
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
        alignment: winding * model_normal.dot(surface_normal.as_vector()).ok()?,
        plane_distance: p
            .vector_to(surface_point)
            .ok()?
            .dot(model_normal)
            .ok()?
            .abs(),
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

/// Find a nearby surface witness for the triangle's center. A replacement
/// witness must stay inside the original pair's parameter quadrilateral.
fn center_distance(
    surface: &NurbsSurface,
    sample: &SurfaceSample,
    region: Option<([Point2; 4], Real)>,
) -> Option<Real> {
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
            if region.is_some_and(|(boundary, winding)| {
                (0..4)
                    .any(|i| wound_side([boundary[i], boundary[(i + 1) % 4], candidate]) != winding)
            }) {
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
    Some(distance)
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
    parameters: &[Point2],
    points: &[Point3],
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
    for i in inverted {
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
            let distance =
                |sample, region| center_distance(surface, sample, region).unwrap_or(Real::INFINITY);
            let prior = distance(&old, None).max(distance(&old_other, None));
            let region = Some((boundary, winding));
            let updated = distance(&new_left, region).max(distance(&new_right, region));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_inverted_lift_of_a_triangle_in_a_convex_parameter_quad() {
        // A regular shear, S(u,v) = (u, v + 2u², 0). Straight chords of
        // one UV diagonal produce a reversed model-space triangle.
        let net = [0., 1.]
            .into_iter()
            .flat_map(|v| {
                [(0., v), (0.5, v), (1., v + 2.)].map(|(x, y)| Point3::try_new(x, y, 0.).unwrap())
            })
            .collect();
        let surface = NurbsSurface::try_new(
            2,
            1,
            3,
            2,
            net,
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let parameters = [(0.2, 0.4), (0.95, 0.35), (0.6, 0.55), (0.65, 0.2)]
            .map(|(u, v)| Point2::try_new(u, v).unwrap());
        let points = parameters.map(|uv| surface.evaluate(uv.x(), uv.y()).unwrap());
        let mut faces = [[0, 1, 2], [1, 0, 3]];
        assert!(
            sample(&surface, &points, &parameters, faces[0])
                .unwrap()
                .alignment
                < 0.
        );

        repair(&surface, &parameters, &points, &mut faces, 3.);

        assert_eq!(faces, [[0, 3, 2], [3, 1, 2]]);
        assert!(faces.iter().all(|&face| {
            sample(&surface, &points, &parameters, face)
                .unwrap()
                .alignment
                > 0.
        }));
    }
}
