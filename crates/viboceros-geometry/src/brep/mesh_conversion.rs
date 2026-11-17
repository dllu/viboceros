//! Exact mesh-to-B-rep conversion and planar n-gon reconstruction.
use super::*;
use crate::{MeshFace, MeshNgon};

#[cfg(test)]
mod tests;

struct PlanarMeshNgon {
    loops: Vec<PlanarMeshNgonLoop>,
    frame: Frame3,
    bounds: [[Real; 2]; 2],
}

struct PlanarMeshNgonLoop {
    vertices: Vec<u32>,
    parameters: Vec<Point2>,
    loop_type: BrepLoopType,
}

enum MeshConversionRegion {
    Face(MeshFace),
    Ngon(PlanarMeshNgon),
}

impl MeshConversionRegion {
    fn loop_count(&self) -> usize {
        match self {
            Self::Face(_) => 1,
            Self::Ngon(ngon) => ngon.loops.len(),
        }
    }

    fn raw_loop(&self, index: usize) -> &[u32] {
        match self {
            Self::Face(face) if index == 0 => face.indices(),
            Self::Ngon(ngon) => &ngon.loops[index].vertices,
            Self::Face(_) => unreachable!("a mesh face has one boundary loop"),
        }
    }
}

impl Brep {
    /// Converts every polygon of a mesh into one degree-one NURBS face.
    ///
    /// Exact-location mesh vertices and edges become shared B-rep topology.
    /// Quads retain their potentially warped bilinear shape. Triangles become
    /// either trimmed planar parallelograms or untrimmed bilinear surfaces
    /// whose west side is collapsed, matching Rhino's `Brep.CreateFromMesh`
    /// construction and parameter domains.
    pub fn try_from_mesh(
        mesh: &TriangleMesh,
        trim_triangular_faces: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_from_mesh_with_ngons(mesh, trim_triangular_faces, false, tolerance)
    }

    /// Converts each planar region of an n-gon into one trimmed planar face
    /// when requested. Faces outside those regions retain their underlying
    /// triangle and quad surfaces.
    pub fn try_from_mesh_with_ngons(
        mesh: &TriangleMesh,
        trim_triangular_faces: bool,
        use_ngons: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut planar_by_first_face = BTreeMap::new();
        let mut covered_faces = if use_ngons && !mesh.ngons().is_empty() {
            vec![false; mesh.face_count()]
        } else {
            Vec::new()
        };
        if use_ngons {
            for ngon in mesh.ngons() {
                for (region_faces, planar) in planar_mesh_ngon_regions(mesh, ngon, tolerance) {
                    let first = *region_faces
                        .iter()
                        .min()
                        .expect("a planar region has faces");
                    planar_by_first_face.insert(first as usize, planar);
                    for face in region_faces {
                        covered_faces[face as usize] = true;
                    }
                }
            }
        }
        let mut regions = Vec::with_capacity(mesh.face_count());
        for (index, &face) in mesh.faces().iter().enumerate() {
            if let Some(ngon) = planar_by_first_face.remove(&index) {
                regions.push(MeshConversionRegion::Ngon(ngon));
            } else if !covered_faces.get(index).copied().unwrap_or(false) {
                regions.push(MeshConversionRegion::Face(face));
            }
        }
        let mut vertex_indices = BTreeMap::<[u64; 3], usize>::new();
        let mut vertices = Vec::new();
        let mut raw_to_brep = Vec::with_capacity(mesh.vertices().len());
        for &point in mesh.vertices() {
            let key = point.to_array().map(canonical_brep_coordinate_bits);
            let index = if let Some(&index) = vertex_indices.get(&key) {
                index
            } else {
                let index = vertices.len();
                vertices.push(BrepVertex::try_new(point, 0.0)?);
                vertex_indices.insert(key, index);
                index
            };
            raw_to_brep.push(index);
        }

        // OpenNURBS builds the complete mesh-topology vertex table, marks
        // vertices touched by faces, and then compacts it. Preserve that
        // order even when an unused raw vertex is the first occurrence of a
        // location referenced again later.
        let mut used_vertices = vec![false; vertices.len()];
        for region in &regions {
            for loop_index in 0..region.loop_count() {
                for &raw in region.raw_loop(loop_index) {
                    used_vertices[raw_to_brep[raw as usize]] = true;
                }
            }
        }
        let mut vertex_remap = vec![usize::MAX; vertices.len()];
        let mut retained_vertices =
            Vec::with_capacity(used_vertices.iter().filter(|&&used| used).count());
        for (source, (vertex, used)) in vertices.into_iter().zip(used_vertices).enumerate() {
            if used {
                vertex_remap[source] = retained_vertices.len();
                retained_vertices.push(vertex);
            }
        }
        for vertex in &mut raw_to_brep {
            *vertex = vertex_remap[*vertex];
        }
        let vertices = retained_vertices;

        let mut edge_use_counts = BTreeMap::<(usize, usize), usize>::new();
        for region in &regions {
            for loop_index in 0..region.loop_count() {
                let indices = region.raw_loop(loop_index);
                for side in 0..indices.len() {
                    let start = raw_to_brep[indices[side] as usize];
                    let end = raw_to_brep[indices[(side + 1) % indices.len()] as usize];
                    let key = ordered_pair(start, end);
                    let uses = edge_use_counts.entry(key).or_default();
                    *uses = uses.checked_add(1).ok_or(GeometryError::TooManyMeshFaces)?;
                }
            }
        }
        let edge_keys = edge_use_counts.keys().copied().collect::<Vec<_>>();
        let edge_indices = edge_keys
            .iter()
            .copied()
            .enumerate()
            .map(|(index, key)| (key, index))
            .collect::<BTreeMap<_, _>>();
        let edges = edge_keys
            .iter()
            .map(|&(start, end)| {
                BrepEdge::try_new(
                    [start, end],
                    NurbsCurve::try_new(
                        1,
                        vec![vertices[start].point, vertices[end].point],
                        vec![0.0, 0.0, 1.0, 1.0],
                    )?,
                    0.0,
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;

        let mut faces = Vec::with_capacity(regions.len());
        for region in &regions {
            if let MeshConversionRegion::Ngon(ngon) = region {
                let surface =
                    planar_cap_surface(ngon.frame, Vector3::try_new(0.0, 0.0, 0.0)?, ngon.bounds)?;
                let mut loops = Vec::with_capacity(ngon.loops.len());
                for boundary in &ngon.loops {
                    let face_vertices = boundary
                        .vertices
                        .iter()
                        .map(|&raw| raw_to_brep[raw as usize])
                        .collect::<Vec<_>>();
                    let mut trims = Vec::with_capacity(face_vertices.len());
                    for side in 0..face_vertices.len() {
                        let next = (side + 1) % face_vertices.len();
                        let start = face_vertices[side];
                        let end = face_vertices[next];
                        let edge_key = ordered_pair(start, end);
                        let edge = edge_indices[&edge_key];
                        let trim_type = if edge_use_counts[&edge_key] == 1 {
                            BrepTrimType::Boundary
                        } else {
                            BrepTrimType::Mated
                        };
                        trims.push(BrepTrim::try_new(
                            [start, end],
                            Some(edge),
                            [start, end] != edges[edge].vertices,
                            NurbsCurve2::try_line(
                                boundary.parameters[side],
                                boundary.parameters[next],
                            )?,
                            trim_type,
                            cap_trim_iso(
                                boundary.parameters[side],
                                boundary.parameters[next],
                                ngon.bounds,
                                tolerance,
                            ),
                            [0.0, 0.0],
                        )?);
                    }
                    loops.push(BrepLoop::try_new(boundary.loop_type, trims)?);
                }
                faces.push(BrepFace::try_new(surface, false, loops)?);
                continue;
            }
            let MeshConversionRegion::Face(face) = region else {
                unreachable!()
            };
            let face_vertices = face
                .indices()
                .iter()
                .map(|&raw| raw_to_brep[raw as usize])
                .collect::<Vec<_>>();
            let face_points = face_vertices
                .iter()
                .map(|&vertex| vertices[vertex].point)
                .collect::<Vec<_>>();
            let surface_corners = match *face {
                MeshFace::Triangle(_) if trim_triangular_faces => {
                    let fourth =
                        face_points[0].translated(face_points[1].vector_to(face_points[2])?)?;
                    [face_points[0], face_points[1], face_points[2], fourth]
                }
                MeshFace::Triangle(_) => [
                    face_points[0],
                    face_points[1],
                    face_points[2],
                    face_points[0],
                ],
                MeshFace::Quad(_) => [
                    face_points[0],
                    face_points[1],
                    face_points[2],
                    face_points[3],
                ],
            };
            let surface = mesh_face_bilinear_surface(surface_corners)?;
            let domain_u = surface.domain_u();
            let domain_v = surface.domain_v();
            let parameters = [
                Point2::try_new(*domain_u.start(), *domain_v.start())?,
                Point2::try_new(*domain_u.end(), *domain_v.start())?,
                Point2::try_new(*domain_u.end(), *domain_v.end())?,
                Point2::try_new(*domain_u.start(), *domain_v.end())?,
            ];
            let (parameter_sides, iso): (&[(usize, usize)], &[SurfaceIso]) = match *face {
                MeshFace::Triangle(_) if trim_triangular_faces => (
                    &[(0, 1), (1, 2), (2, 0)],
                    &[SurfaceIso::South, SurfaceIso::East, SurfaceIso::NotIso],
                ),
                MeshFace::Triangle(_) => (
                    &[(0, 1), (1, 2), (2, 3)],
                    &[SurfaceIso::South, SurfaceIso::East, SurfaceIso::North],
                ),
                MeshFace::Quad(_) => (
                    &[(0, 1), (1, 2), (2, 3), (3, 0)],
                    &[
                        SurfaceIso::South,
                        SurfaceIso::East,
                        SurfaceIso::North,
                        SurfaceIso::West,
                    ],
                ),
            };
            let mut trims = Vec::with_capacity(
                parameter_sides.len() + usize::from(face.is_triangle() && !trim_triangular_faces),
            );
            for (side, (&(parameter_start, parameter_end), &iso)) in
                parameter_sides.iter().zip(iso).enumerate()
            {
                let start = face_vertices[side];
                let end = face_vertices[(side + 1) % face_vertices.len()];
                let edge_key = ordered_pair(start, end);
                let edge = edge_indices[&edge_key];
                let trim_type = if edge_use_counts[&edge_key] == 1 {
                    BrepTrimType::Boundary
                } else {
                    BrepTrimType::Mated
                };
                trims.push(BrepTrim::try_new(
                    [start, end],
                    Some(edge),
                    [start, end] != edges[edge].vertices,
                    NurbsCurve2::try_line(parameters[parameter_start], parameters[parameter_end])?,
                    trim_type,
                    iso,
                    [0.0, 0.0],
                )?);
            }
            if face.is_triangle() && !trim_triangular_faces {
                trims.push(BrepTrim::try_new(
                    [face_vertices[0], face_vertices[0]],
                    None,
                    false,
                    NurbsCurve2::try_line(parameters[3], parameters[0])?,
                    BrepTrimType::Singular,
                    SurfaceIso::West,
                    [0.0, 0.0],
                )?);
            }
            faces.push(BrepFace::try_new(
                surface,
                false,
                vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
            )?);
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }
}

fn planar_mesh_ngon_regions(
    mesh: &TriangleMesh,
    ngon: &MeshNgon,
    tolerance: Tolerance,
) -> Vec<(Vec<u32>, PlanarMeshNgon)> {
    if let Some(planar) = planar_mesh_ngon(mesh, ngon, tolerance) {
        return vec![(ngon.faces().to_vec(), planar)];
    }

    // A validated n-gon has opposite edge uses on every internal edge. Build
    // only those face adjacencies, then grow regions against the seed plane.
    // Keeping the seed fixed prevents a tolerance-sized fold at each edge
    // from accumulating into an arbitrarily nonplanar merged face.
    let mut edge_faces = BTreeMap::<(u32, u32), Vec<u32>>::new();
    for &face in ngon.faces() {
        let indices = mesh.faces()[face as usize].indices();
        for side in 0..indices.len() {
            let a = indices[side];
            let b = indices[(side + 1) % indices.len()];
            edge_faces
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push(face);
        }
    }
    let mut neighbors = BTreeMap::<u32, Vec<u32>>::new();
    for faces in edge_faces.values() {
        if let [a, b] = faces.as_slice() {
            neighbors.entry(*a).or_default().push(*b);
            neighbors.entry(*b).or_default().push(*a);
        }
    }
    let mut visited = BTreeSet::new();
    let mut regions = Vec::new();
    for &seed in ngon.faces() {
        if !visited.insert(seed) {
            continue;
        }
        let seed_indices = mesh.faces()[seed as usize].indices();
        let [origin, along_x, in_xy] = [seed_indices[0], seed_indices[1], seed_indices[2]]
            .map(|raw| mesh.vertices()[raw as usize]);
        let Ok(frame) = Frame3::try_from_points(origin, along_x, in_xy, tolerance) else {
            continue;
        };
        if !mesh_face_on_plane(mesh, seed, frame, tolerance) {
            continue;
        }
        let mut queue = VecDeque::from([seed]);
        let mut faces = vec![seed];
        while let Some(face) = queue.pop_front() {
            for &neighbor in neighbors.get(&face).into_iter().flatten() {
                if !visited.contains(&neighbor)
                    && mesh_face_on_plane(mesh, neighbor, frame, tolerance)
                {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                    faces.push(neighbor);
                }
            }
        }
        if faces.len() < 2 {
            continue;
        }
        faces.sort_unstable();
        let planar = mesh
            .ngon_from_faces(faces.clone())
            .and_then(|region| planar_mesh_ngon(mesh, &region, tolerance))
            .or_else(|| planar_mesh_region_with_holes(mesh, &faces, frame, tolerance));
        if let Some(planar) = planar {
            regions.push((faces, planar));
        }
    }
    regions
}

fn mesh_face_on_plane(mesh: &TriangleMesh, face: u32, frame: Frame3, tolerance: Tolerance) -> bool {
    mesh.faces()[face as usize].indices().iter().all(|&raw| {
        frame
            .coordinates_of(mesh.vertices()[raw as usize])
            .is_ok_and(|coordinates| coordinates[2].abs() <= tolerance.absolute())
    })
}

fn planar_mesh_ngon(
    mesh: &TriangleMesh,
    ngon: &MeshNgon,
    tolerance: Tolerance,
) -> Option<PlanarMeshNgon> {
    let boundary = ngon.vertices();
    let origin = mesh.vertices()[boundary[0] as usize];
    let along_x = mesh.vertices()[boundary[1] as usize];
    let mut frame = boundary[2..].iter().find_map(|&raw| {
        Frame3::try_from_points(origin, along_x, mesh.vertices()[raw as usize], tolerance).ok()
    })?;
    for &face in ngon.faces() {
        for &raw in mesh.faces()[face as usize].indices() {
            if frame.coordinates_of(mesh.vertices()[raw as usize]).ok()?[2].abs()
                > tolerance.absolute()
            {
                return None;
            }
        }
    }
    if mesh.ngon_boundary_loops(ngon.faces())?.len() > 1 {
        return planar_mesh_region_with_holes(mesh, ngon.faces(), frame, tolerance);
    }
    let project = |frame: Frame3| {
        boundary
            .iter()
            .map(|&raw| {
                let [x, y] = frame
                    .projected_coordinates_of(mesh.vertices()[raw as usize])
                    .ok()?;
                Point2::try_new(x, y).ok()
            })
            .collect::<Option<Vec<_>>>()
    };
    let mut parameters = project(frame)?;
    let signed_area = parameters
        .iter()
        .zip(parameters.iter().cycle().skip(1))
        .map(|(a, b)| a.x().mul_add(b.y(), -b.x() * a.y()))
        .sum::<Real>();
    if !signed_area.is_finite() || signed_area == 0.0 {
        return None;
    }
    if signed_area < 0.0 {
        frame = Frame3::try_from_x_and_normal(
            origin,
            origin.vector_to(along_x).ok()?,
            frame.z_axis().as_vector().scaled(-1.0).ok()?,
            tolerance,
        )
        .ok()?;
        parameters = project(frame)?;
    }
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    for point in &parameters {
        bounds[0][0] = bounds[0][0].min(point.x());
        bounds[0][1] = bounds[0][1].max(point.x());
        bounds[1][0] = bounds[1][0].min(point.y());
        bounds[1][1] = bounds[1][1].max(point.y());
    }
    if bounds.iter().any(|axis| {
        !axis[0].is_finite() || !axis[1].is_finite() || axis[1] - axis[0] <= tolerance.absolute()
    }) {
        return None;
    }
    Some(PlanarMeshNgon {
        loops: vec![PlanarMeshNgonLoop {
            vertices: boundary.to_vec(),
            parameters,
            loop_type: BrepLoopType::Outer,
        }],
        frame,
        bounds,
    })
}

fn planar_mesh_region_with_holes(
    mesh: &TriangleMesh,
    faces: &[u32],
    mut frame: Frame3,
    tolerance: Tolerance,
) -> Option<PlanarMeshNgon> {
    if faces
        .iter()
        .any(|&face| !mesh_face_on_plane(mesh, face, frame, tolerance))
    {
        return None;
    }
    let mut edge_uses = BTreeMap::<(u32, u32), Vec<(u32, u32)>>::new();
    for &face in faces {
        let indices = mesh.faces()[face as usize].indices();
        for side in 0..indices.len() {
            let from = indices[side];
            let to = indices[(side + 1) % indices.len()];
            edge_uses
                .entry((from.min(to), from.max(to)))
                .or_default()
                .push((from, to));
        }
    }
    let mut next = BTreeMap::<u32, u32>::new();
    let mut incoming = BTreeSet::new();
    for uses in edge_uses.values() {
        match uses.as_slice() {
            [(from, to)] => {
                if next.insert(*from, *to).is_some() || !incoming.insert(*to) {
                    return None;
                }
            }
            [(a, b), (c, d)] if *a == *d && *b == *c => {}
            _ => return None,
        }
    }
    if next.len() < 6 || next.len() != incoming.len() {
        return None;
    }
    let mut boundaries = Vec::new();
    while let Some((&start, _)) = next.first_key_value() {
        let mut boundary = Vec::new();
        let mut current = start;
        loop {
            boundary.push(current);
            let following = next.remove(&current)?;
            if following == start {
                break;
            }
            current = following;
        }
        if boundary.len() < 3 {
            return None;
        }
        boundaries.push(boundary);
    }
    if boundaries.len() < 2 {
        return None;
    }
    let project = |frame: Frame3, boundary: &[u32]| {
        boundary
            .iter()
            .map(|&raw| {
                let [x, y] = frame
                    .projected_coordinates_of(mesh.vertices()[raw as usize])
                    .ok()?;
                Point2::try_new(x, y).ok()
            })
            .collect::<Option<Vec<_>>>()
    };
    let mut projected = boundaries
        .iter()
        .map(|boundary| project(frame, boundary))
        .collect::<Option<Vec<_>>>()?;
    let signed_area = |points: &[Point2]| {
        points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .map(|(a, b)| a.x().mul_add(b.y(), -b.x() * a.y()))
            .sum::<Real>()
    };
    let areas = projected
        .iter()
        .map(|points| signed_area(points))
        .collect::<Vec<_>>();
    if areas.iter().any(|area| !area.is_finite() || *area == 0.0) {
        return None;
    }
    let outer = (0..areas.len()).max_by(|&a, &b| areas[a].abs().total_cmp(&areas[b].abs()))?;
    if areas[outer] < 0.0 {
        frame = Frame3::try_from_x_and_normal(
            frame.origin(),
            frame.x_axis().as_vector(),
            frame.z_axis().as_vector().scaled(-1.0).ok()?,
            tolerance,
        )
        .ok()?;
        projected = boundaries
            .iter()
            .map(|boundary| project(frame, boundary))
            .collect::<Option<Vec<_>>>()?;
    }
    if projected.iter().enumerate().any(|(index, points)| {
        let area = signed_area(points);
        if index == outer {
            area <= 0.0
        } else {
            area >= 0.0
        }
    }) {
        return None;
    }
    let mut loops = boundaries
        .into_iter()
        .zip(projected)
        .enumerate()
        .map(|(index, (vertices, parameters))| PlanarMeshNgonLoop {
            vertices,
            parameters,
            loop_type: if index == outer {
                BrepLoopType::Outer
            } else {
                BrepLoopType::Inner
            },
        })
        .collect::<Vec<_>>();
    loops.swap(0, outer);
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    let mut parameters = Vec::new();
    let mut loop_lengths = Vec::with_capacity(loops.len());
    for boundary in &loops {
        loop_lengths.push(boundary.parameters.len());
        for &point in &boundary.parameters {
            bounds[0][0] = bounds[0][0].min(point.x());
            bounds[0][1] = bounds[0][1].max(point.x());
            bounds[1][0] = bounds[1][0].min(point.y());
            bounds[1][1] = bounds[1][1].max(point.y());
            parameters.push(point);
        }
    }
    if bounds.iter().any(|axis| {
        !axis[0].is_finite() || !axis[1].is_finite() || axis[1] - axis[0] <= tolerance.absolute()
    }) || triangulate_trim_region(&parameters, &loop_lengths)
        .ok()
        .flatten()
        .is_none()
    {
        return None;
    }
    Some(PlanarMeshNgon {
        loops,
        frame,
        bounds,
    })
}

fn mesh_face_bilinear_surface(corners: [Point3; 4]) -> Result<NurbsSurface, GeometryError> {
    const OPENNURBS_ZERO_TOLERANCE: Real = 2.328_306_436_538_696_3e-10;
    let mut domain_u_end = corners[0]
        .distance_to(corners[1])?
        .max(corners[3].distance_to(corners[2])?);
    if domain_u_end <= OPENNURBS_ZERO_TOLERANCE {
        domain_u_end = 1.0;
    }
    let mut domain_v_end = corners[1]
        .distance_to(corners[2])?
        .max(corners[0].distance_to(corners[3])?);
    if domain_v_end <= OPENNURBS_ZERO_TOLERANCE {
        domain_v_end = 1.0;
    }
    NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![corners[0], corners[1], corners[3], corners[2]],
        vec![0.0, 0.0, domain_u_end, domain_u_end],
        vec![0.0, 0.0, domain_v_end, domain_v_end],
    )
}
