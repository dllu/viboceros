//! Constrained triangulation of spatial samples in a fitted plane or surface chart.
use super::*;
use crate::exact_scalar::{Rational, rational};
use num_traits::Zero;
use spade::{
    ConstrainedDelaunayTriangulation, HasPosition, Point2 as TriangulationPoint2, Triangulation,
};
use std::cmp::Ordering;
use std::ops::Range;
mod constraints;
#[cfg(test)]
mod tests;

pub const MAX_MESH_PATCH_POINTS: usize = 100_000;

#[derive(Clone, Copy, Debug)]
pub struct MeshPatchOptions<'a> {
    pub trim_back: bool,
    pub divisions: usize,
    pub starting_surface: Option<&'a NurbsSurface>,
    pub outer_boundary: Option<&'a Polyline3>,
}
impl Default for MeshPatchOptions<'_> {
    fn default() -> Self {
        Self {
            trim_back: true,
            divisions: 1,
            starting_surface: None,
            outer_boundary: None,
        }
    }
}

#[derive(Clone, Debug)]
struct Vertex {
    position: TriangulationPoint2<Real>,
    source: usize,
    point: Point3,
}
impl HasPosition for Vertex {
    type Scalar = Real;
    fn position(&self) -> TriangulationPoint2<Real> {
        self.position
    }
}

fn boundary_orientation(points: &[[Real; 2]]) -> Ordering {
    let approximate = projected_polygon_doubled_area(points);
    if approximate.abs() > 64. * Real::EPSILON * points.len() as Real {
        return approximate.total_cmp(&0.);
    }
    let mut area = Rational::zero();
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        area += rational(a[0]) * rational(b[1]) - rational(a[1]) * rational(b[0]);
    }
    area.cmp(&Rational::zero())
}

impl TriangleMesh {
    /// Builds a constrained Delaunay patch, retaining original spatial samples.
    /// Guides constrain edges on both sides. With automatic trimming, guide
    /// regions containing input points survive; without points, enclosed guide
    /// regions survive. The temporary rectangular perimeter is then removed.
    /// Holes and an explicit outer boundary trim connected triangulation regions.
    /// A starting surface supplies closest-point UV coordinates, not a universal
    /// unfolding: conflicting chart locations and unresolved boundaries fail.
    /// With automatic trimming, unsplit guides recover before paths through
    /// explicit point seeds.
    /// Earlier recovered edges take priority without generated spatial samples.
    /// Failed recovery trims adjacent endpoint faces; incomplete
    /// holes trim only the faces beside their recovered boundary edges.
    pub fn try_patch(
        points: &[Point3],
        guides: &[Polyline3],
        holes: &[Polyline3],
        options: MeshPatchOptions<'_>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let invalid = |context| GeometryError::Degenerate { context };
        let automatic_trim = options.trim_back && options.outer_boundary.is_none();
        if holes
            .iter()
            .chain(options.outer_boundary)
            .any(|c| c.vertices().first() != c.vertices().last())
        {
            return Err(invalid("mesh patch boundary must be exactly closed"));
        }
        if options.divisions == 0 || options.divisions > MAX_MESH_PATCH_POINTS / 4 {
            return Err(invalid("mesh patch perimeter divisions"));
        }
        let count = guides
            .iter()
            .chain(holes)
            .chain(options.outer_boundary)
            .try_fold(points.len(), |n, c| n.checked_add(c.vertices().len()))
            .ok_or(GeometryError::MeshPatchLimit)?;
        let divisions = if options.trim_back {
            1
        } else {
            options.divisions
        };
        if count > MAX_MESH_PATCH_POINTS
            || options.outer_boundary.is_none()
                && count.saturating_add(divisions * 4) > MAX_MESH_PATCH_POINTS
        {
            return Err(GeometryError::MeshPatchLimit);
        }
        let mut all = points.to_vec();
        let mut chains = Vec::new();
        for (curve, barrier) in guides
            .iter()
            .map(|c| (c, automatic_trim))
            .chain(holes.iter().map(|c| (c, true)))
            .chain(options.outer_boundary.map(|c| (c, true)))
        {
            if barrier && !automatic_trim && curve.vertices().first() != curve.vertices().last() {
                return Err(invalid("mesh patch boundary must be exactly closed"));
            }
            let begin = all.len();
            all.extend_from_slice(curve.vertices());
            chains.push((begin..all.len(), barrier));
            if all.len() > MAX_MESH_PATCH_POINTS {
                return Err(GeometryError::MeshPatchLimit);
            }
        }
        if all.len() < 3 {
            return Err(invalid("mesh patch needs three noncollinear locations"));
        }
        if all.len() > MAX_MESH_PATCH_POINTS {
            return Err(GeometryError::MeshPatchLimit);
        }
        let plane = PointProjection3::onto_best_fit_plane(&all)?.as_plane()?;
        let frame = Frame3::try_from_normal(
            plane.origin(),
            plane.normal().as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let project = |point| -> Result<[Real; 2], GeometryError> {
            if let Some(surface) = options.starting_surface {
                let (u, v) = surface.closest_parameters(point, tolerance)?;
                Ok([u, v])
            } else {
                frame.projected_coordinates_of(point)
            }
        };
        let mut projected = all
            .iter()
            .copied()
            .map(project)
            .collect::<Result<Vec<_>, _>>()?;
        let mut lo = [Real::INFINITY; 2];
        let mut hi = [Real::NEG_INFINITY; 2];
        for q in &projected {
            for axis in 0..2 {
                lo[axis] = lo[axis].min(q[axis]);
                hi[axis] = hi[axis].max(q[axis]);
            }
        }
        let source_count = all.len();
        if options.outer_boundary.is_none() {
            if let Some(surface) = options.starting_surface {
                lo = [*surface.domain_u().start(), *surface.domain_v().start()];
                hi = [*surface.domain_u().end(), *surface.domain_v().end()];
            } else {
                for axis in 0..2 {
                    let margin = (hi[axis] - lo[axis]) * 0.1;
                    lo[axis] -= margin;
                    hi[axis] += margin;
                }
            }
            for (a, b) in [
                ([lo[0], lo[1]], [hi[0], lo[1]]),
                ([hi[0], lo[1]], [hi[0], hi[1]]),
                ([hi[0], hi[1]], [lo[0], hi[1]]),
                ([lo[0], hi[1]], [lo[0], lo[1]]),
            ] {
                for i in 0..divisions {
                    let f = i as Real / divisions as Real;
                    let q = std::array::from_fn(|axis| (b[axis] - a[axis]).mul_add(f, a[axis]));
                    let p = if let Some(surface) = options.starting_surface {
                        surface.evaluate(q[0], q[1])?
                    } else {
                        frame.point_at([q[0], q[1], 0.])?
                    };
                    all.push(p);
                    projected.push(q);
                }
            }
        }
        if all.len() > MAX_MESH_PATCH_POINTS {
            return Err(GeometryError::MeshPatchLimit);
        }
        let origin = projected[0];
        let scale = projected
            .iter()
            .flat_map(|q| (0..2).map(move |a| (q[a] - origin[a]).abs()))
            .fold(0., Real::max);
        if scale == 0. || !scale.is_finite() {
            return Err(invalid("mesh patch chart scale"));
        }
        let mut keys = BTreeMap::new();
        let mut vertices = Vec::new();
        let mut coordinates = Vec::new();
        let mut remap = Vec::new();
        for (point, q) in all.into_iter().zip(projected) {
            let q = std::array::from_fn::<_, 2, _>(|a| (q[a] - origin[a]) / scale);
            let key = q.map(canonical_coordinate_bits);
            let id = if let Some(&id) = keys.get(&key) {
                if point.distance_to(vertices[id])? > tolerance.absolute() {
                    return Err(GeometryError::MeshPatchProjectionConflict);
                }
                id
            } else {
                let id = vertices.len();
                keys.insert(key, id);
                vertices.push(point);
                coordinates.push(q);
                id
            };
            remap.push(id);
        }
        let input = coordinates
            .iter()
            .enumerate()
            .map(|(source, q)| Vertex {
                position: TriangulationPoint2::new(q[0], q[1]),
                source,
                point: vertices[source],
            })
            .collect();
        let mut triangulation = ConstrainedDelaunayTriangulation::<Vertex>::bulk_load(input)
            .map_err(|_| invalid("mesh patch triangulation coordinates"))?;
        if triangulation.num_inner_faces() == 0 {
            return Err(invalid("mesh patch needs three noncollinear locations"));
        }
        let source_nodes = remap[..points.len()]
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let constraints = constraints::Constraints::install(
            &mut triangulation,
            &coordinates,
            &remap,
            &chains,
            (guides.len(), holes.len()),
            automatic_trim,
            &source_nodes,
        )?;
        let mut neighbors = vec![Vec::new(); triangulation.num_all_faces()];
        for edge in triangulation.directed_edges() {
            if !constraints.barriers.contains(&edge.as_undirected().index()) {
                neighbors[edge.face().index()].push(edge.rev().face().index());
            }
        }
        let mut removed = vec![false; neighbors.len()];
        let mut queue = VecDeque::new();
        if options.outer_boundary.is_some() {
            queue.push_back(triangulation.outer_face().index());
        }
        for &(edge, left) in &constraints.holes {
            let edge = triangulation.directed_edge(edge);
            queue.push_back(if left {
                edge.face().index()
            } else {
                edge.rev().face().index()
            });
        }
        while let Some(face) = queue.pop_front() {
            if !removed[face] {
                removed[face] = true;
                queue.extend(neighbors[face].iter().copied());
            }
        }
        let perimeter_nodes = remap[source_count..]
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let outer_seeds = if automatic_trim && !points.is_empty() {
            constraints.outer_seeds(&triangulation, &remap, source_count)?
        } else {
            vec![]
        };
        let mut has_point = vec![false; neighbors.len()];
        let mut has_perimeter = vec![false; neighbors.len()];
        has_perimeter[triangulation.outer_face().index()] = true;
        for face in triangulation.inner_faces() {
            has_point[face.index()] = face
                .vertices()
                .iter()
                .any(|v| source_nodes.contains(&v.data().source));
            has_perimeter[face.index()] = face
                .vertices()
                .iter()
                .any(|v| perimeter_nodes.contains(&v.data().source));
        }
        for face in outer_seeds {
            has_point[face] = true;
        }
        let mut reaches_perimeter = vec![false; neighbors.len()];
        if automatic_trim {
            let mut visited = vec![false; neighbors.len()];
            for seed in 0..neighbors.len() {
                if visited[seed] {
                    continue;
                }
                let mut component = vec![seed];
                visited[seed] = true;
                let mut cursor = 0;
                let mut seeded = false;
                let mut perimeter = false;
                while cursor < component.len() {
                    let face = component[cursor];
                    cursor += 1;
                    seeded |= has_point[face];
                    perimeter |= has_perimeter[face];
                    for &next in &neighbors[face] {
                        if !visited[next] {
                            visited[next] = true;
                            component.push(next);
                        }
                    }
                }
                let keep = if source_nodes.is_empty() {
                    !perimeter
                } else {
                    seeded
                };
                for face in component {
                    removed[face] |= !keep;
                    reaches_perimeter[face] = perimeter;
                }
            }
        }
        constraints.mark_partial_faces(&triangulation, &source_nodes, &mut removed);
        let triangles = triangulation
            .inner_faces()
            .filter(|f| !removed[f.index()])
            .filter(|f| {
                !reaches_perimeter[f.index()]
                    || !f
                        .adjacent_edges()
                        .iter()
                        .any(|e| constraints.thin_guides.contains(&e.as_undirected().index()))
            })
            .filter(|f| {
                !options.trim_back
                    || !f
                        .vertices()
                        .iter()
                        .any(|v| perimeter_nodes.contains(&v.data().source))
            })
            .map(|f| f.vertices().map(|v| v.data().source as u32))
            // A fitted chart can create numerical slivers between collinear
            // spatial samples. Keep the existing mesh validation policy.
            .filter(|face| {
                !matches!(
                    validate_triangle(&vertices, *face, 0, false, Tolerance::MESH_VALIDATION),
                    Err(GeometryError::DegenerateTriangle { .. })
                )
            })
            .collect::<Vec<_>>();
        if triangles.is_empty() {
            return Err(GeometryError::EmptyMeshPatch);
        }
        Ok(
            Self::try_new(vertices, triangles, Tolerance::MESH_VALIDATION)?
                .culled_unused_vertices()
                .0,
        )
    }
}
