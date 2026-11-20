//! Retriangulate a three-face neighborhood when a single flip cannot repair it.
use super::{
    Edge, HashMap, NO_FACE, NurbsSurface, Point2, Point3, Real, Tolerance, Triangle,
    center_distance, chords, edge, sample, wound_side,
};

type Adjacency = HashMap<Edge, [usize; 2]>;

fn directed_edges([a, b, c]: Triangle) -> [Edge; 3] {
    [[a, b], [b, c], [c, a]]
}

fn neighbors(i: usize, faces: &[Triangle], adjacent: &Adjacency) -> [Option<usize>; 3] {
    directed_edges(faces[i]).map(|[a, b]| {
        let [first, second] = adjacent[&edge(a, b)];
        let other = if first == i { second } else { first };
        (other != NO_FACE).then_some(other)
    })
}

fn boundary(faces: [Triangle; 3]) -> Option<[u32; 5]> {
    let edges = faces.map(directed_edges).into_iter().flatten();
    let outer: [Edge; 5] = edges
        .clone()
        .filter(|&[a, b]| !edges.clone().any(|other| other == [b, a]))
        .collect::<Vec<_>>()
        .try_into()
        .ok()?;
    let mut ring = [outer[0][0]; 5];
    for i in 1..5 {
        let [_, next] = *outer.iter().find(|[a, _]| *a == ring[i - 1])?;
        if ring[..i].contains(&next) {
            return None;
        }
        ring[i] = next;
    }
    outer.contains(&[ring[4], ring[0]]).then_some(ring)
}

fn install(
    faces: &mut [Triangle],
    adjacent: &mut Adjacency,
    indices: [usize; 3],
    replacement: [Triangle; 3],
) {
    for i in indices {
        for [a, b] in directed_edges(faces[i]) {
            let key = edge(a, b);
            let pair = adjacent.get_mut(&key).expect("edge of an installed face");
            pair[usize::from(pair[0] != i)] = NO_FACE;
            if *pair == [NO_FACE; 2] {
                adjacent.remove(&key);
            }
        }
    }
    for (i, face) in indices.into_iter().zip(replacement) {
        faces[i] = face;
        for [a, b] in directed_edges(face) {
            let pair = adjacent.entry(edge(a, b)).or_insert([NO_FACE; 2]);
            pair[usize::from(pair[0] != NO_FACE)] = i;
        }
    }
}

struct SurfaceMesh<'a> {
    surface: &'a NurbsSurface,
    parameters: &'a [Point2],
    points: &'a [Point3],
    height: Real,
}

impl SurfaceMesh<'_> {
    fn uv(&self, triangle: Triangle) -> [Point2; 3] {
        triangle.map(|i| self.parameters[i as usize])
    }

    fn chord_error(&self, [a, b]: Edge) -> Option<Real> {
        let [p, q] = [a, b].map(|i| self.parameters[i as usize]);
        let [left, right] = [a, b].map(|i| self.points[i as usize]);
        [0.25, 0.5, 0.75]
            .into_iter()
            .try_fold(0., |error: Real, t| {
                let site = self
                    .surface
                    .evaluate(p.x() + (q.x() - p.x()) * t, p.y() + (q.y() - p.y()) * t)
                    .ok()?;
                Some(error.max(chords::distance_to_chord(site, left, right).ok()?))
            })
    }

    fn best_partition(
        &self,
        indices: [usize; 3],
        faces: &[Triangle],
        adjacent: &Adjacency,
    ) -> Option<(Real, [Triangle; 3])> {
        let old = indices.map(|i| faces[i]);
        let ring = boundary(old)?;
        let area = old.map(|face| self.uv(face));
        let winding = wound_side(area[0]);
        if winding == 0. || area.iter().any(|&uv| wound_side(uv) != winding) {
            return None;
        }
        // These triangles form a disk in the conforming UV mesh. All five
        // combinatorial partitions of its pentagon are fans. Matching the
        // boundary and the strict winding keeps the filled UV region fixed,
        // including in concave cases.
        let sample = |face| sample(self.surface, self.points, self.parameters, face);
        let [Some(a), Some(b), Some(c)] = old.map(sample) else {
            return None;
        };
        let previous = [a, b, c];
        let inside = |p| {
            area.iter().any(|&[a, b, c]| {
                [[a, b, p], [b, c, p], [c, a, p]]
                    .into_iter()
                    .all(|side| wound_side(side) == winding)
            })
        };
        let plane_limit = previous
            .iter()
            .map(|s| s.plane_distance)
            .fold(self.height / 12., Real::max);
        let mut distance_limit = None;
        let mut best = None;
        for i in 0..5 {
            let v = |j: usize| ring[(i + j) % 5];
            let internal = [edge(v(0), v(2)), edge(v(0), v(3))];
            let mut error: Real = 0.;
            for key in internal {
                let chord = match adjacent.get(&key) {
                    Some(owners) if owners.iter().all(|i| indices.contains(i)) => 0.,
                    Some(_) => Real::INFINITY,
                    None => self.chord_error(key).unwrap_or(Real::INFINITY),
                };
                error = error.max(chord);
            }
            if error > self.height || best.is_some_and(|(distance, _)| error >= distance) {
                continue;
            }
            let candidate = [[v(0), v(1), v(2)], [v(0), v(2), v(3)], [v(0), v(3), v(4)]];
            if candidate.iter().any(|&face| {
                wound_side(self.uv(face)) != winding
                    || !crate::nurbs_surface::tessellation_triangle_is_nondegenerate(
                        self.points,
                        face,
                        Tolerance::MESH_VALIDATION,
                    )
                    .unwrap_or(false)
            }) {
                continue;
            }
            let [Some(a), Some(b), Some(c)] = candidate.map(sample) else {
                continue;
            };
            let current = [a, b, c];
            if current
                .iter()
                .any(|s| s.alignment <= 0.1 || s.plane_distance > plane_limit)
            {
                continue;
            }
            let limit = *distance_limit.get_or_insert_with(|| {
                previous
                    .iter()
                    .map(|s| center_distance(self.surface, s, |_| true).unwrap_or(Real::INFINITY))
                    .fold(self.height / 24., Real::max)
            });
            if limit.is_finite()
                && current.iter().all(|s| {
                    center_distance(self.surface, s, inside)
                        .is_some_and(|distance| distance <= limit)
                })
            {
                best = Some((error, candidate));
            }
        }
        best
    }
}

pub(super) fn repair(
    surface: &NurbsSurface,
    parameters: &[Point2],
    points: &[Point3],
    faces: &mut [Triangle],
    height: Real,
    inverted: &[usize],
    adjacent: &mut Adjacency,
) {
    let mesh = SurfaceMesh {
        surface,
        parameters,
        points,
        height,
    };
    for &i in inverted {
        if !sample(surface, points, parameters, faces[i]).is_some_and(|s| s.alignment < 0.) {
            continue;
        }
        let first = neighbors(i, faces, adjacent);
        let mut tried = Vec::new();
        let mut best = None;
        for j in first.into_iter().flatten() {
            for k in first
                .into_iter()
                .chain(neighbors(j, faces, adjacent))
                .flatten()
            {
                if k == i || k == j {
                    continue;
                }
                let mut indices = [i, j, k];
                indices.sort_unstable();
                if tried.contains(&indices) {
                    continue;
                }
                tried.push(indices);
                if let Some((error, replacement)) = mesh.best_partition(indices, faces, adjacent)
                    && best.is_none_or(|(prior, _, _)| error < prior)
                {
                    best = Some((error, indices, replacement));
                }
            }
        }
        if let Some((_, indices, replacement)) = best {
            install(faces, adjacent, indices, replacement);
        }
    }
}
