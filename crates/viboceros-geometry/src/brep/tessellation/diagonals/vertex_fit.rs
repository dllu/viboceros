//! Relocate interior display vertices when diagonal changes leave a local fold.
use super::*;
use crate::brep::tessellation::audit::key as vertex_location;
use std::collections::HashSet;

pub(super) fn relocate(
    surface: &NurbsSurface,
    parameters: &mut [Point2],
    points: &mut [Point3],
    triangles: &[Triangle],
    height: Real,
    previously_inverted: &[usize],
    adjacent: &HashMap<Edge, [usize; 2]>,
) {
    let mut inverted: Vec<_> = previously_inverted
        .iter()
        .filter_map(|&face| {
            sample(surface, points, parameters, triangles[face])
                .filter(|s| s.alignment < 0.)
                .map(|s| (face, s.twice_area))
        })
        .collect();
    if inverted.is_empty() {
        return;
    }
    inverted.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let boundary: HashSet<_> = adjacent
        .iter()
        .filter(|(_, incidence)| incidence[1] == NO_FACE)
        .flat_map(|(&edge, _)| edge)
        .collect();
    let mut stars: HashMap<_, Vec<Triangle>> = inverted
        .iter()
        .flat_map(|&(i, _)| triangles[i])
        .filter(|vertex| !boundary.contains(vertex))
        .map(|vertex| (vertex, Vec::new()))
        .collect();
    if stars.is_empty() {
        return;
    }
    for &triangle in triangles {
        for vertex in triangle {
            if let Some(star) = stars.get_mut(&vertex) {
                star.push(triangle);
            }
        }
    }
    let mut mesh = Mesh {
        surface,
        positions: points,
        uv: parameters,
        height,
        locations: HashMap::new(),
    };
    for &point in mesh.positions.iter() {
        *mesh.locations.entry(vertex_location(point)).or_insert(0) += 1;
    }
    for (face, _) in inverted {
        if mesh
            .sample(triangles[face])
            .is_none_or(|value| value.alignment >= 0.)
        {
            continue;
        }
        let best = triangles[face]
            .into_iter()
            .filter_map(|v| mesh.fit(v, stars.get(&v)?))
            .max_by(|a, b| a.improvement.total_cmp(&b.improvement));
        if let Some(best) = best {
            let vertex = best.vertex;
            mesh.locations
                .remove(&vertex_location(mesh.positions[vertex]));
            mesh.locations.insert(vertex_location(best.point), 1);
            mesh.positions[vertex] = best.point;
            mesh.uv[vertex] = best.uv;
        }
    }
}

struct Mesh<'a> {
    surface: &'a NurbsSurface,
    positions: &'a mut [Point3],
    uv: &'a mut [Point2],
    height: Real,
    locations: HashMap<[u64; 3], usize>,
}

struct Fit {
    vertex: usize,
    uv: Point2,
    point: Point3,
    improvement: Real,
}

impl Mesh<'_> {
    fn sample(&self, triangle: Triangle) -> Option<SurfaceSample> {
        sample(self.surface, self.positions, self.uv, triangle)
    }

    fn triangle_uv(&self, triangle: Triangle) -> [Point2; 3] {
        triangle.map(|index| self.uv[index as usize])
    }

    fn chord_error(&self, vertex: u32, neighbors: &[u32]) -> Option<Real> {
        let mut error: Real = 0.;
        let a = self.uv[vertex as usize];
        let p = self.positions[vertex as usize];
        for &neighbor in neighbors {
            let b = self.uv[neighbor as usize];
            let q = self.positions[neighbor as usize];
            for t in [0.25, 0.5, 0.75] {
                let point = self
                    .surface
                    .evaluate(a.x() + t * (b.x() - a.x()), a.y() + t * (b.y() - a.y()))
                    .ok()?;
                error = error.max(chords::distance_to_chord(point, p, q).ok()?);
            }
        }
        Some(error)
    }

    fn fit(&mut self, vertex: u32, star: &[Triangle]) -> Option<Fit> {
        let index = vertex as usize;
        let old_point = self.positions[index];
        let old_uv = self.uv[index];
        if self.locations[&vertex_location(old_point)] != 1 {
            return None;
        }
        let mut neighbors: Vec<_> = star
            .iter()
            .flatten()
            .copied()
            .filter(|&v| v != vertex)
            .collect();
        neighbors.sort_unstable();
        neighbors.dedup();
        if neighbors.len() != star.len() || star.len() < 3 {
            return None;
        }
        let area: Vec<_> = star
            .iter()
            .map(|&t| signed_area(self.triangle_uv(t)))
            .collect();
        if area.contains(&0.) {
            return None;
        }
        let before: Vec<_> = star
            .iter()
            .map(|&t| self.sample(t))
            .collect::<Option<_>>()?;
        let folded_area: Real = before
            .iter()
            .filter(|s| s.alignment < 0.)
            .map(|s| s.twice_area)
            .sum();
        let mut plane_limit = self.height / 12.;
        let mut center_limit = self.height / 24.;
        for s in &before {
            plane_limit = plane_limit.max(s.plane_distance);
            center_limit = center_limit.max(center_distance(self.surface, s, |_| true)?);
        }
        let chord_limit = self.chord_error(vertex, &neighbors)?.max(self.height / 8.);
        let mean = |coordinate: fn(Point2) -> Real| {
            neighbors
                .iter()
                .map(|&v| coordinate(self.uv[v as usize]))
                .sum::<Real>()
                / neighbors.len() as Real
        };
        let target = Point2::try_new(mean(Point2::x), mean(Point2::y)).ok()?;
        let mut best: Option<Fit> = None;
        for fraction in [0.25, 0.5, 0.75, 1.] {
            let Ok(uv) = Point2::try_new(
                old_uv.x() + fraction * (target.x() - old_uv.x()),
                old_uv.y() + fraction * (target.y() - old_uv.y()),
            ) else {
                continue;
            };
            let Ok(point) = self.surface.evaluate(uv.x(), uv.y()) else {
                continue;
            };
            if self.locations.contains_key(&vertex_location(point)) {
                continue;
            }
            self.uv[index] = uv;
            self.positions[index] = point;
            let check = || {
                let inside = |p| {
                    star.iter().zip(&area).any(|(&triangle, old_area)| {
                        let [a, b, c] = self.triangle_uv(triangle);
                        [[a, b, p], [b, c, p], [c, a, p]]
                            .into_iter()
                            .all(|side| signed_area(side) * old_area >= 0.)
                    })
                };
                let mut value = 0.;
                for ((&triangle, &old_area), old) in star.iter().zip(&area).zip(&before) {
                    // Keep the whole triangle fan in its original UV polygon.
                    let area = signed_area(self.triangle_uv(triangle));
                    if area / old_area < 0.1 {
                        return None;
                    }
                    if !crate::nurbs_surface::tessellation_triangle_is_nondegenerate(
                        self.positions,
                        triangle,
                        Tolerance::MESH_VALIDATION,
                    )
                    .ok()?
                    {
                        return None;
                    }
                    let current = self.sample(triangle)?;
                    if current.alignment < 0. {
                        if old.alignment >= 0. {
                            return None;
                        }
                        value += current.twice_area;
                    }
                    if current.plane_distance > plane_limit
                        || center_distance(self.surface, &current, inside)? > center_limit
                    {
                        return None;
                    }
                }
                (self.chord_error(vertex, &neighbors)? <= chord_limit).then_some(value)
            };
            if let Some(value) = check() {
                let improvement = folded_area - value;
                if improvement > 0. && best.as_ref().is_none_or(|b| improvement > b.improvement) {
                    best = Some(Fit {
                        vertex: index,
                        uv,
                        point,
                        improvement,
                    });
                }
            }
        }
        self.uv[index] = old_uv;
        self.positions[index] = old_point;
        best
    }
}
