//! Planar mesh caps with an outer boundary and one or more inner boundaries.
use super::*;

/// Reuses source raw vertices on newly capped boundaries. Only vertices at
/// locations used by a new cap are welded; unrelated open boundaries and
/// existing interior seams retain their original raw indices.
pub(super) fn weld_cap_boundary(
    source: &TriangleMesh,
    capped: TriangleMesh,
    tolerance: Tolerance,
) -> Result<TriangleMesh, GeometryError> {
    let source_len = source.vertices.len();
    debug_assert_eq!(&capped.vertices[..source_len], source.vertices.as_slice());
    let mut representatives = capped.vertices[source_len..]
        .iter()
        .copied()
        .map(|point| (point_key(point), u32::MAX))
        .collect::<BTreeMap<_, _>>();
    let data = source.topology_data();
    for use_record in data
        .edges
        .values()
        .filter(|incidence| incidence.count == 1)
        .filter_map(|incidence| incidence.first_use)
    {
        for raw in use_record.raw_vertices {
            let key = point_key(source.vertices[raw as usize]);
            if let Some(representative) = representatives.get_mut(&key) {
                *representative = (*representative).min(raw);
            }
        }
    }
    if representatives.values().any(|&raw| raw == u32::MAX) {
        return Err(GeometryError::MeshCapWeldSourceMissing);
    }

    let mut remap = (0..capped.vertices.len())
        .map(|index| u32::try_from(index).expect("a validated mesh has u32 vertex indices"))
        .collect::<Vec<_>>();
    for use_record in data
        .edges
        .values()
        .filter(|incidence| incidence.count == 1)
        .filter_map(|incidence| incidence.first_use)
    {
        for raw in use_record.raw_vertices {
            let key = point_key(source.vertices[raw as usize]);
            if let Some(&representative) = representatives.get(&key) {
                remap[raw as usize] = representative;
            }
        }
    }
    for (index, point) in capped.vertices[source_len..].iter().copied().enumerate() {
        remap[source_len + index] = representatives[&point_key(point)];
    }
    let faces = capped
        .faces
        .into_iter()
        .map(|face| face.remapped(|raw| remap[raw as usize]))
        .collect();
    let mut vertices = capped.vertices;
    vertices.truncate(source_len);
    TriangleMesh::try_new_faces(vertices, faces, tolerance)
}

pub(super) fn try_cap_annular(
    mesh: &TriangleMesh,
    data: &MeshTopologyData,
    tolerance: Tolerance,
) -> Result<Option<(TriangleMesh, usize)>, GeometryError> {
    let loops = mesh
        .boundary_polylines(Tolerance::MESH_VALIDATION)?
        .into_iter()
        .filter(Polyline3::is_closed)
        .map(|polyline| polyline.vertices()[..polyline.vertices().len() - 1].to_vec())
        .collect::<Vec<_>>();
    if loops.len() < 2 {
        return Ok(None);
    }
    let bounds = loops
        .iter()
        .map(|loop_points| BoundingBox3::from_points(loop_points.iter().copied()))
        .collect::<Result<Vec<_>, _>>()?;
    let topology_points = data
        .topological_points
        .iter()
        .copied()
        .enumerate()
        .map(|(index, point)| (point_key(point), index))
        .collect::<BTreeMap<_, _>>();
    for outer_index in 0..loops.len() {
        let outer = &loops[outer_index];
        let plane = PointProjection3::onto_best_fit_plane(outer)?;
        if !is_on_plane(outer, &plane, tolerance)? {
            continue;
        }
        let Some(frame) = Projection::new(outer)? else {
            continue;
        };
        let outer_projected = outer
            .iter()
            .copied()
            .map(|point| frame.project(point))
            .collect::<Result<Vec<_>, _>>()?;
        let outer_area = projected_polygon_doubled_area(&outer_projected);
        let epsilon = 64.0 * Real::EPSILON * outer.len() as Real;
        if outer_area.abs() <= epsilon {
            continue;
        }

        let mut holes = Vec::new();
        let mut ambiguous = false;
        for other_index in 0..loops.len() {
            if other_index == outer_index
                || !bounds_overlap(bounds[outer_index], bounds[other_index])
            {
                continue;
            }
            let other = &loops[other_index];
            if !is_on_plane(other, &plane, tolerance)? {
                continue;
            }
            let projected = other
                .iter()
                .copied()
                .map(|point| frame.project(point))
                .collect::<Result<Vec<_>, _>>()?;
            let area = projected_polygon_doubled_area(&projected);
            if area.abs() < outer_area.abs()
                && projected
                    .iter()
                    .copied()
                    .all(|point| point_in_mesh_hole_polygon(point, &outer_projected, epsilon))
            {
                holes.push((other_index, projected));
            } else {
                // Intersecting, touching, and nonnested coplanar loops need a
                // joint region classifier; never build overlapping disks.
                ambiguous = true;
                break;
            }
        }
        if ambiguous || holes.is_empty() {
            continue;
        }
        if mesh_has_coplanar_face_in_bounds(mesh, &plane, bounds[outer_index], tolerance)? {
            continue;
        }

        let Some(source_follows_outer) = source_follows_loop(outer, data, &topology_points) else {
            continue;
        };
        let desired_positive = (outer_area > 0.0) != source_follows_outer;
        if holes.iter().any(|(index, projected)| {
            let Some(source_follows) = source_follows_loop(&loops[*index], data, &topology_points)
            else {
                return true;
            };
            ((projected_polygon_doubled_area(projected) > 0.0) != source_follows)
                == desired_positive
        }) {
            continue;
        }

        let mut points = outer.to_vec();
        let mut projected = outer_projected;
        let mut ranges = Vec::with_capacity(holes.len() + 1);
        ranges.push(0..outer.len());
        for (index, hole_projection) in &holes {
            let start = points.len();
            points.extend(loops[*index].iter().copied());
            projected.extend(hole_projection.iter().copied());
            ranges.push(start..points.len());
        }
        let Some(mut triangles) = triangulate_projected_mesh_region(&projected, &ranges)? else {
            continue;
        };
        if !desired_positive {
            for triangle in &mut triangles {
                triangle.swap(1, 2);
            }
        }
        let total_vertices = mesh
            .vertices
            .len()
            .checked_add(points.len())
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if u32::try_from(total_vertices - 1).is_err() {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let offset = u32::try_from(mesh.vertices.len())
            .expect("the checked appended vertex range starts within u32");
        let mut vertices = mesh.vertices.clone();
        vertices
            .try_reserve(points.len())
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        vertices.extend(points);
        let mut faces = mesh.faces.clone();
        faces
            .try_reserve(triangles.len())
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        faces.extend(
            triangles
                .into_iter()
                .map(|triangle| MeshFace::Triangle(triangle.map(|index| offset + index))),
        );
        return TriangleMesh::try_new_faces(vertices, faces, tolerance)
            .map(|capped| Some((capped, holes.len() + 1)));
    }
    Ok(None)
}

fn is_on_plane(
    points: &[Point3],
    plane: &PointProjection3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    points.iter().copied().try_fold(true, |on_plane, point| {
        Ok(on_plane && point.distance_to(plane.project(point)?)? <= tolerance.absolute())
    })
}

fn bounds_overlap(first: BoundingBox3, second: BoundingBox3) -> bool {
    let (first_min, first_max) = (first.min().to_array(), first.max().to_array());
    let (second_min, second_max) = (second.min().to_array(), second.max().to_array());
    (0..3).all(|axis| first_min[axis] <= second_max[axis] && second_min[axis] <= first_max[axis])
}

fn mesh_has_coplanar_face_in_bounds(
    mesh: &TriangleMesh,
    plane: &PointProjection3,
    bounds: BoundingBox3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    for face in &mesh.faces {
        let points = face
            .indices()
            .iter()
            .map(|&index| mesh.vertices[index as usize])
            .collect::<Vec<_>>();
        if bounds_overlap(bounds, BoundingBox3::from_points(points.iter().copied())?)
            && is_on_plane(&points, plane, tolerance)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn point_key(point: Point3) -> [u64; 3] {
    point.to_array().map(canonical_coordinate_bits)
}

fn source_follows_loop(
    points: &[Point3],
    data: &MeshTopologyData,
    topology_points: &BTreeMap<[u64; 3], usize>,
) -> Option<bool> {
    let mut aligned = None;
    for index in 0..points.len() {
        let first = *topology_points.get(&point_key(points[index]))?;
        let second = *topology_points.get(&point_key(points[(index + 1) % points.len()]))?;
        let key = (first.min(second), first.max(second));
        let incidence = data.edges.get(&key)?;
        if incidence.count != 1 {
            return None;
        }
        let follows = incidence.first_use?.forward == (first < second);
        if aligned.is_some_and(|value| value != follows) {
            return None;
        }
        aligned = Some(follows);
    }
    aligned
}

struct Projection {
    origin: [Real; 3],
    global_scale: Option<Real>,
    scale: Real,
    axes: [usize; 2],
}

impl Projection {
    fn new(points: &[Point3]) -> Result<Option<Self>, GeometryError> {
        let origin = points[0].to_array();
        let global_scale = points
            .iter()
            .flat_map(|point| point.to_array())
            .map(Real::abs)
            .fold(0.0, Real::max);
        let direct = points.iter().all(|point| {
            point
                .to_array()
                .into_iter()
                .zip(origin)
                .all(|(value, origin)| (value - origin).is_finite())
        });
        let global_scale = if direct { None } else { Some(global_scale) };
        let mut frame = Self {
            origin,
            global_scale,
            scale: 1.0,
            axes: [0, 1],
        };
        let relative = points
            .iter()
            .copied()
            .map(|point| frame.relative(point))
            .collect::<Vec<_>>();
        frame.scale = relative
            .iter()
            .flatten()
            .map(|value| value.abs())
            .fold(0.0, Real::max);
        if frame.scale == 0.0 {
            return Ok(None);
        }
        let normalized = relative
            .iter()
            .map(|point| point.map(|value| value / frame.scale))
            .collect::<Vec<_>>();
        let mut best = 0.0;
        for axes in [[0, 1], [1, 2], [2, 0]] {
            let polygon = normalized
                .iter()
                .map(|point| [point[axes[0]], point[axes[1]]])
                .collect::<Vec<_>>();
            let area = projected_polygon_doubled_area(&polygon).abs();
            if area > best {
                best = area;
                frame.axes = axes;
            }
        }
        if best <= 64.0 * Real::EPSILON * points.len() as Real {
            return Ok(None);
        }
        Ok(Some(frame))
    }

    fn relative(&self, point: Point3) -> [Real; 3] {
        let values = point.to_array();
        std::array::from_fn(|axis| {
            if let Some(scale) = self.global_scale {
                values[axis] / scale - self.origin[axis] / scale
            } else {
                values[axis] - self.origin[axis]
            }
        })
    }

    fn project(&self, point: Point3) -> Result<[Real; 2], GeometryError> {
        let relative = self.relative(point);
        let projected = [
            relative[self.axes[0]] / self.scale,
            relative[self.axes[1]] / self.scale,
        ];
        require_finite(projected, "mesh planar cap projection")?;
        Ok(projected)
    }
}
