//! Insert a surface vertex into a small cavity whose lifted triangles fold.

use super::*;
use crate::brep::tessellation::audit::key as vertex_location;
use cavity::{Adjacency, attach, boundary, detach, neighbors};
use nalgebra::Vector2;
use std::collections::HashSet;

type Uv = Vector2<Real>;

// Clip a convex polygon against the positive side of an affine function.
fn clip(polygon: &mut Vec<Uv>, normal: Uv, offset: Real) {
    let old = std::mem::take(polygon);
    let Some(&last) = old.last() else {
        return;
    };
    let mut a = last;
    let mut fa = normal.dot(&a) - offset;
    for b in old {
        let fb = normal.dot(&b) - offset;
        if (fa >= 0.) != (fb >= 0.) {
            polygon.push(a + (b - a) * (fa / (fa - fb)));
        }
        if fb >= 0. {
            polygon.push(b);
        }
        a = b;
        fa = fb;
    }
}

fn mean(polygon: &[Uv]) -> Option<Uv> {
    (polygon.len() >= 3).then(|| polygon.iter().sum::<Uv>() / polygon.len() as Real)
}

fn inside(p: Point2, triangles: &[[Point2; 3]]) -> bool {
    triangles.iter().any(|&[a, b, c]| {
        [[a, b, p], [b, c, p], [c, a, p]]
            .into_iter()
            .all(|side| wound_side(side) >= 0.)
    })
}

fn alignment_at(surface: &NurbsSurface, site: Point2, normal: Vector3) -> Option<Real> {
    let (_, du, dv) = surface.evaluate_with_derivatives(site.x(), site.y()).ok()?;
    let reference = du.cross(dv).ok()?.normalized_nonzero().ok()?;
    reference.as_vector().dot(normal).ok()
}

struct DisplayPatch<'a> {
    surface: &'a NurbsSurface,
    uv: &'a mut Vec<Point2>,
    points: &'a mut Vec<Point3>,
    height: Real,
    locations: HashSet<[u64; 3]>,
}

impl DisplayPatch<'_> {
    fn sample(&self, triangle: Triangle) -> Option<SurfaceSample> {
        // The conforming triangulator directs its faces CCW in UV. A very
        // thin triangle's stored parameters may lose its computed winding.
        sample_in_winding(self.surface, self.points, self.uv, triangle, 1.)
    }

    fn reversed(&self, sample: &SurfaceSample) -> bool {
        if sample.alignment < 0. {
            return true;
        }
        // An oblique triangle can oppose the surface nearest its center
        // even when it agrees with the analytic normal at the UV centroid.
        sample.alignment <= std::f64::consts::FRAC_1_SQRT_2
            && center_witness(self.surface, sample, |_| true)
                .and_then(|(uv, _)| alignment_at(self.surface, uv, sample.normal))
                .is_some_and(|alignment| alignment < 0.)
    }

    fn fan<const N: usize, const B: usize>(
        &mut self,
        old: [Triangle; N],
    ) -> Option<(Point2, Point3, [Triangle; B])> {
        let ring = boundary::<N, B>(old)?;
        let chart = old.map(|t| t.map(|i| self.uv[i as usize]));
        let boundary_uv = ring.map(|i| Uv::from(self.uv[i as usize].to_array()));
        let lo = boundary_uv.iter().fold(boundary_uv[0], |a, p| a.inf(p));
        let hi = boundary_uv.iter().fold(boundary_uv[0], |a, p| a.sup(p));
        let span = hi - lo;
        if span.min() <= 0. || !span.iter().all(|x| x.is_finite()) {
            return None;
        }
        let extent = span.x * span.y;
        let areas = chart.map(signed_area);
        if areas.iter().any(|a| *a < -256. * Real::EPSILON * extent) {
            return None;
        }
        let area = areas.into_iter().sum::<Real>();
        if !area.is_finite() || area <= 256. * Real::EPSILON * extent {
            return None;
        }

        // The visibility kernel keeps every spoke inside the original UV
        // polygon. A small positive area prevents degenerate replacements.
        let boundary_uv = boundary_uv.map(|uv| (uv - lo).component_div(&span));
        let mut kernel = vec![
            Uv::new(0., 0.),
            Uv::new(1., 0.),
            Uv::new(1., 1.),
            Uv::new(0., 1.),
        ];
        for i in 0..B {
            let edge = boundary_uv[(i + 1) % B] - boundary_uv[i];
            let side = Uv::new(-edge.y, edge.x);
            clip(
                &mut kernel,
                side,
                side.dot(&boundary_uv[i]) + 0.005 * area / extent,
            );
        }
        let mut proposed = mean(&kernel)?;
        let mut planes = Vec::with_capacity(B);
        for i in 0..B {
            let [a, b] = [ring[i], ring[(i + 1) % B]].map(|j| j as usize);
            let [u, v] = [self.uv[a], self.uv[b]];
            let (_, du, dv) = self
                .surface
                .evaluate_with_derivatives(u.x().midpoint(v.x()), u.y().midpoint(v.y()))
                .ok()?;
            let edge = self.points[a].vector_to(self.points[b]).ok()?;
            let inward = du
                .cross(dv)
                .ok()?
                .cross(edge)
                .ok()?
                .normalized_nonzero()
                .ok()?;
            planes.push((self.points[a], inward.as_vector()));
        }

        let previous = old.map(|face| self.sample(face));
        if previous.iter().any(Option::is_none) {
            return None;
        }
        let previous = previous.map(Option::unwrap);
        let plane_limit = previous
            .iter()
            .map(|s| s.plane_distance)
            .fold(self.height / 12., Real::max);
        let distance_limit = previous.iter().try_fold(self.height / 24., |limit, s| {
            Some(limit.max(center_distance(self.surface, s, |_| true)?))
        })?;
        let vertex = self.points.len() as u32;
        let replacement = std::array::from_fn(|i| [ring[i], ring[(i + 1) % B], vertex]);
        for _ in 0..5 {
            let [u, v]: [Real; 2] = (lo + proposed.component_mul(&span)).into();
            let uv = Point2::try_new(u, v).ok()?;
            let (point, du, dv) = self.surface.evaluate_with_derivatives(u, v).ok()?;
            if !self.locations.contains(&vertex_location(point)) {
                self.points.push(point);
                self.uv.push(uv);
                let valid = self.approves(&replacement, &chart, plane_limit, distance_limit);
                self.uv.pop();
                self.points.pop();
                if valid {
                    return Some((uv, point, replacement));
                }
            }
            // Linearize the physical inward halfplanes at the current
            // surface point and seek their intersection with the UV kernel.
            let mut feasible = kernel.clone();
            for &(origin, inward) in &planes {
                let displacement = origin.vector_to(point).ok()?.dot(inward).ok()?;
                let normal = Uv::new(du.dot(inward).ok()? * span.x, dv.dot(inward).ok()? * span.y);
                clip(&mut feasible, normal, normal.dot(&proposed) - displacement);
            }
            let next = mean(&feasible)?;
            if (next - proposed).norm_squared() <= 1e-12 {
                break;
            }
            proposed = next;
        }
        None
    }

    fn approves(
        &self,
        replacement: &[Triangle],
        chart: &[[Point2; 3]],
        plane_limit: Real,
        distance_limit: Real,
    ) -> bool {
        replacement.iter().all(|&triangle| {
            let [a, b, m] = triangle;
            if wound_side(triangle.map(|i| self.uv[i as usize])) <= 0.
                || !crate::nurbs_surface::tessellation_triangle_is_nondegenerate(
                    self.points,
                    triangle,
                    Tolerance::MESH_VALIDATION,
                )
                .unwrap_or(false)
            {
                return false;
            }
            let Some(sample) = self.sample(triangle) else {
                return false;
            };
            if sample.alignment < 0.1 || sample.plane_distance > plane_limit {
                return false;
            }
            let Some((_, distance)) = center_witness(self.surface, &sample, |p| inside(p, chart))
            else {
                return false;
            };
            if distance > distance_limit {
                return false;
            }
            if self.reversed(&sample) {
                return false;
            }
            [a, b].into_iter().all(|i| {
                chord_deviation(self.surface, self.points, self.uv, [i, m])
                    .is_some_and(|error| error <= self.height)
            })
        })
    }

    fn insert<const N: usize, const B: usize>(
        &mut self,
        indices: [usize; N],
        faces: &mut Vec<Triangle>,
        adjacent: &mut Adjacency,
    ) -> bool {
        let Some((uv, point, replacement)) = self.fan::<N, B>(indices.map(|i| faces[i])) else {
            return false;
        };
        self.points.push(point);
        self.uv.push(uv);
        self.locations.insert(vertex_location(point));
        for i in indices {
            detach(adjacent, i, faces[i]);
        }
        for (j, face) in replacement.into_iter().enumerate() {
            let i = if j < N {
                faces[indices[j]] = face;
                indices[j]
            } else {
                faces.push(face);
                faces.len() - 1
            };
            attach(adjacent, i, face);
        }
        true
    }
}

pub(in super::super) fn repair_remaining(
    surface: &NurbsSurface,
    uv: &mut Vec<Point2>,
    points: &mut Vec<Point3>,
    faces: &mut Vec<Triangle>,
    height: Real,
    limit: usize,
) {
    if points.len() >= limit {
        return;
    }
    let mut patch = DisplayPatch {
        surface,
        uv,
        points,
        height,
        locations: HashSet::new(),
    };
    let mut suspect = faces
        .iter()
        .enumerate()
        .filter_map(|(i, &face)| {
            patch
                .sample(face)
                .filter(|s| patch.reversed(s))
                .map(|s| (i, s.twice_area))
        })
        .collect::<Vec<_>>();
    if suspect.is_empty() {
        return;
    }
    suspect.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut adjacent = Adjacency::with_capacity(faces.len());
    for (i, &face) in faces.iter().enumerate() {
        if !attach(&mut adjacent, i, face) {
            return;
        }
    }
    patch
        .locations
        .extend(patch.points.iter().copied().map(vertex_location));
    for (i, _) in suspect {
        if patch.points.len() >= limit {
            break;
        }
        if !patch
            .sample(faces[i])
            .is_some_and(|sample| patch.reversed(&sample))
        {
            continue;
        }
        let ring = neighbors(i, faces, &adjacent);
        if ring
            .into_iter()
            .flatten()
            .any(|j| patch.insert::<2, 4>([i, j], faces, &mut adjacent))
        {
            continue;
        }
        'pairs: for j in ring.into_iter().flatten() {
            for k in ring
                .into_iter()
                .chain(neighbors(j, faces, &adjacent))
                .flatten()
            {
                if k != i && k != j && patch.insert::<3, 5>([i, j, k], faces, &mut adjacent) {
                    break 'pairs;
                }
            }
        }
    }
}
