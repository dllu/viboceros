//! Retriangulate small neighborhoods when a single flip cannot repair them.
use super::{
    Edge, HashMap, NO_FACE, NurbsSurface, Point2, Point3, Real, SurfaceSample, Tolerance, Triangle,
    center_distance, chords, edge, sample, wound_side,
};

mod hexagon;

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

fn boundary<const N: usize, const B: usize>(faces: [Triangle; N]) -> Option<[u32; B]> {
    let edges = faces.map(directed_edges).into_iter().flatten();
    let outer: [Edge; B] = edges
        .clone()
        .filter(|&[a, b]| !edges.clone().any(|other| other == [b, a]))
        .collect::<Vec<_>>()
        .try_into()
        .ok()?;
    let mut ring = [outer[0][0]; B];
    for i in 1..B {
        let [_, next] = *outer.iter().find(|[a, _]| *a == ring[i - 1])?;
        if ring[..i].contains(&next) {
            return None;
        }
        ring[i] = next;
    }
    outer.contains(&[ring[B - 1], ring[0]]).then_some(ring)
}

fn install<const N: usize>(
    faces: &mut [Triangle],
    adjacent: &mut Adjacency,
    indices: [usize; N],
    replacement: [Triangle; N],
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

#[derive(Default)]
struct Probes {
    chords: HashMap<Edge, Real>,
    triangles: HashMap<Triangle, Option<SurfaceSample>>,
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

    fn best_partition<const N: usize, const B: usize>(
        &self,
        indices: [usize; N],
        faces: &[Triangle],
        adjacent: &Adjacency,
        probes: &mut Probes,
        templates: &[[[usize; 3]; N]],
    ) -> Option<(Real, [Triangle; N])> {
        let old = indices.map(|i| faces[i]);
        let ring = boundary::<N, B>(old)?;
        let area = old.map(|face| self.uv(face));
        let winding = wound_side(area[0]);
        if winding == 0. || area.iter().any(|&uv| wound_side(uv) != winding) {
            return None;
        }
        // Matching the boundary and strict winding preserves the filled UV
        // region. This also rejects invalid partitions of concave polygons.
        let mut sample = |face| {
            *probes
                .triangles
                .entry(face)
                .or_insert_with(|| sample(self.surface, self.points, self.parameters, face))
        };
        let previous = old.map(&mut sample);
        if previous.iter().any(Option::is_none) {
            return None;
        }
        let inside = |p| {
            area.iter().any(|&[a, b, c]| {
                [[a, b, p], [b, c, p], [c, a, p]]
                    .into_iter()
                    .all(|side| wound_side(side) == winding)
            })
        };
        let plane_limit = previous
            .iter()
            .flatten()
            .map(|s| s.plane_distance)
            .fold(self.height / 12., Real::max);
        let mut distance_limit = None;
        let mut best = None;
        for &template in templates {
            let candidate = template.map(|face| face.map(|i| ring[i]));
            // Discard fans that leave the original UV region before evaluating
            // the surface along their proposed diagonals.
            if candidate
                .iter()
                .any(|&face| wound_side(self.uv(face)) != winding)
            {
                continue;
            }
            let internal = template
                .into_iter()
                .flat_map(|[a, b, c]| [[a, b], [b, c], [c, a]])
                .filter(|&[a, b]| a < b && b - a > 1 && b - a < B - 1);
            let error = internal.fold(0., |error: Real, [a, b]| {
                let key = edge(ring[a], ring[b]);
                let chord = match adjacent.get(&key) {
                    Some(owners) if owners.iter().all(|i| indices.contains(i)) => 0.,
                    Some(_) => Real::INFINITY,
                    None => *probes
                        .chords
                        .entry(key)
                        .or_insert_with(|| self.chord_error(key).unwrap_or(Real::INFINITY)),
                };
                error.max(chord)
            });
            if error > self.height || best.is_some_and(|(distance, _)| error >= distance) {
                continue;
            }
            if candidate.iter().any(|&face| {
                !crate::nurbs_surface::tessellation_triangle_is_nondegenerate(
                    self.points,
                    face,
                    Tolerance::MESH_VALIDATION,
                )
                .unwrap_or(false)
            }) {
                continue;
            }
            let current = candidate.map(&mut sample);
            if current
                .iter()
                .any(|s| s.is_none_or(|s| s.alignment <= 0.1 || s.plane_distance > plane_limit))
            {
                continue;
            }
            let limit = *distance_limit.get_or_insert_with(|| {
                previous
                    .iter()
                    .flatten()
                    .map(|s| center_distance(self.surface, s, |_| true).unwrap_or(Real::INFINITY))
                    .fold(self.height / 24., Real::max)
            });
            if limit.is_finite()
                && current.iter().flatten().all(|s| {
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

// A pentagon's five partitions are fans from its five vertices.
const PENTAGONS: [[[usize; 3]; 3]; 5] = {
    let mut patterns = [[[0; 3]; 3]; 5];
    let mut i = 0;
    while i < patterns.len() {
        let mut j = 0;
        while j < 3 {
            patterns[i][j] = [i, (i + j + 1) % 5, (i + j + 2) % 5];
            j += 1;
        }
        i += 1;
    }
    patterns
};

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
    // Both passes reuse probes for overlapping cavities. The vertices and
    // surface remain fixed while partitions change the connectivity.
    let mut probes = Probes::default();
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
                if let Some((error, replacement)) =
                    mesh.best_partition::<3, 5>(indices, faces, adjacent, &mut probes, &PENTAGONS)
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
    hexagon::repair(&mesh, faces, inverted, adjacent, &mut probes);
}
