use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::vector::product_three;
use crate::{
    AffineTransform3, BoundingBox3, GeometryError, LineSegment, Point3, Polyline3, Real, Tolerance,
    UnitVector3, require_finite,
};

/// Exact location-welded edge topology for a polygon mesh.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshTopology {
    topological_vertex_count: usize,
    edge_count: usize,
    boundary_edge_count: usize,
    non_manifold_edge_count: usize,
    orientation_conflict_edge_count: usize,
    closed: bool,
}

impl MeshTopology {
    pub const fn topological_vertex_count(self) -> usize {
        self.topological_vertex_count
    }

    pub const fn edge_count(self) -> usize {
        self.edge_count
    }

    pub const fn boundary_edge_count(self) -> usize {
        self.boundary_edge_count
    }

    pub const fn non_manifold_edge_count(self) -> usize {
        self.non_manifold_edge_count
    }

    pub const fn orientation_conflict_edge_count(self) -> usize {
        self.orientation_conflict_edge_count
    }

    pub const fn is_closed(self) -> bool {
        self.closed
    }

    pub const fn is_manifold(self) -> bool {
        self.non_manifold_edge_count == 0
    }

    pub const fn is_oriented(self) -> bool {
        self.non_manifold_edge_count == 0 && self.orientation_conflict_edge_count == 0
    }

    pub const fn is_solid(self) -> bool {
        self.is_closed() && self.is_manifold() && self.is_oriented()
    }
}

#[derive(Clone, Debug, Default)]
struct EdgeIncidence {
    count: usize,
    forward_count: usize,
    first_use: Option<EdgeUse>,
    second_use: Option<EdgeUse>,
    additional_uses: Vec<EdgeUse>,
}

impl EdgeIncidence {
    fn add_use(&mut self, edge_use: EdgeUse) {
        match self.count {
            0 => self.first_use = Some(edge_use),
            1 => self.second_use = Some(edge_use),
            _ => self.additional_uses.push(edge_use),
        }
        self.count += 1;
        self.forward_count += usize::from(edge_use.forward);
    }

    fn uses(&self) -> impl Iterator<Item = EdgeUse> + '_ {
        self.first_use
            .into_iter()
            .chain(self.second_use)
            .chain(self.additional_uses.iter().copied())
    }
}

#[derive(Clone, Copy, Debug)]
struct EdgeUse {
    face: usize,
    forward: bool,
}

#[derive(Debug)]
struct MeshTopologyData {
    topological_vertex_count: usize,
    topological_points: Vec<Point3>,
    edges: BTreeMap<(usize, usize), EdgeIncidence>,
}

/// The two parts produced by extracting faces from a mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshFaceExtraction {
    remainder: Option<TriangleMesh>,
    extracted: TriangleMesh,
}

impl MeshFaceExtraction {
    pub fn remainder(&self) -> Option<&TriangleMesh> {
        self.remainder.as_ref()
    }

    pub const fn extracted(&self) -> &TriangleMesh {
        &self.extracted
    }

    pub fn into_parts(self) -> (Option<TriangleMesh>, TriangleMesh) {
        (self.remainder, self.extracted)
    }
}

/// One validated triangle or quadrilateral in a polygon mesh.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MeshFace {
    Triangle([u32; 3]),
    Quad([u32; 4]),
}

impl MeshFace {
    #[inline]
    pub const fn vertex_count(self) -> usize {
        match self {
            Self::Triangle(_) => 3,
            Self::Quad(_) => 4,
        }
    }

    #[inline]
    pub const fn is_triangle(self) -> bool {
        matches!(self, Self::Triangle(_))
    }

    #[inline]
    pub const fn is_quad(self) -> bool {
        matches!(self, Self::Quad(_))
    }

    #[inline]
    pub fn indices(&self) -> &[u32] {
        match self {
            Self::Triangle(indices) => indices,
            Self::Quad(indices) => indices,
        }
    }

    fn reversed(self) -> Self {
        match self {
            Self::Triangle([a, b, c]) => Self::Triangle([a, c, b]),
            Self::Quad([a, b, c, d]) => Self::Quad([a, d, c, b]),
        }
    }

    fn remapped(self, mut map: impl FnMut(u32) -> u32) -> Self {
        match self {
            Self::Triangle(indices) => Self::Triangle(indices.map(&mut map)),
            Self::Quad(indices) => Self::Quad(indices.map(&mut map)),
        }
    }
}

/// An indexed, oriented polygon mesh with validated finite vertices and
/// non-degenerate triangle and quadrilateral faces.
///
/// `TriangleMesh` retains its original public name for API compatibility, but
/// quadrilateral faces remain first-class so topology and 3DM interchange do
/// not invent diagonal edges. [`Self::triangles`] provides a deterministic
/// `0-2` triangulation for algorithms and formats that require triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct TriangleMesh {
    vertices: Vec<Point3>,
    faces: Vec<MeshFace>,
    triangles: Vec<[u32; 3]>,
}

impl TriangleMesh {
    pub fn try_new(
        vertices: Vec<Point3>,
        triangles: Vec<[u32; 3]>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_new_faces(
            vertices,
            triangles.into_iter().map(MeshFace::Triangle).collect(),
            tolerance,
        )
    }

    pub fn try_new_faces(
        vertices: Vec<Point3>,
        faces: Vec<MeshFace>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if faces.is_empty() {
            return Err(GeometryError::EmptyMesh);
        }
        if vertices
            .len()
            .checked_sub(1)
            .is_some_and(|last_index| u32::try_from(last_index).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }

        let mut triangles = Vec::new();
        triangles
            .try_reserve(faces.len().saturating_mul(2))
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for (face_index, face) in faces.iter().copied().enumerate() {
            match face {
                MeshFace::Triangle(triangle) => {
                    validate_triangle(&vertices, triangle, face_index, false, tolerance)?;
                    triangles.push(triangle);
                }
                MeshFace::Quad([a, b, c, d]) => {
                    validate_triangle(&vertices, [a, b, c], face_index, true, tolerance)?;
                    validate_triangle(&vertices, [a, c, d], face_index, true, tolerance)?;
                    if vertices[b as usize] == vertices[d as usize] {
                        return Err(GeometryError::DegenerateQuad { face: face_index });
                    }
                    triangles.extend([[a, b, c], [a, c, d]]);
                }
            }
        }

        Ok(Self {
            vertices,
            faces,
            triangles,
        })
    }

    fn from_validated_parts(vertices: Vec<Point3>, faces: Vec<MeshFace>) -> Self {
        let mut triangles = Vec::with_capacity(faces.len().saturating_mul(2));
        for face in &faces {
            match *face {
                MeshFace::Triangle(triangle) => triangles.push(triangle),
                MeshFace::Quad([a, b, c, d]) => {
                    triangles.extend([[a, b, c], [a, c, d]]);
                }
            }
        }
        Self {
            vertices,
            faces,
            triangles,
        }
    }

    #[inline]
    pub fn vertices(&self) -> &[Point3] {
        &self.vertices
    }

    #[inline]
    pub fn faces(&self) -> &[MeshFace] {
        &self.faces
    }

    #[inline]
    pub const fn face_count(&self) -> usize {
        self.faces.len()
    }

    #[inline]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// Reverses every face winding without changing mesh vertex or face order.
    pub fn reversed(&self) -> Self {
        Self::from_validated_parts(
            self.vertices.clone(),
            self.faces.iter().copied().map(MeshFace::reversed).collect(),
        )
    }

    pub fn triangle_points(&self, index: usize) -> Option<[Point3; 3]> {
        let triangle = *self.triangles.get(index)?;
        Some([
            self.vertices[triangle[0] as usize],
            self.vertices[triangle[1] as usize],
            self.vertices[triangle[2] as usize],
        ])
    }

    /// Builds OpenNURBS-compatible edge topology after welding vertices at
    /// exactly equal 3D locations. This recognizes indexed meshes and
    /// triangle-soup imports consistently without applying model tolerance.
    pub fn topology(&self) -> MeshTopology {
        let data = self.topology_data();
        let edges = &data.edges;
        let boundary_edge_count = edges
            .values()
            .filter(|incidence| incidence.count == 1)
            .count();
        let non_manifold_edge_count = edges
            .values()
            .filter(|incidence| incidence.count > 2)
            .count();
        let orientation_conflict_edge_count = edges
            .values()
            .filter(|incidence| {
                incidence.count == 2
                    && (incidence.forward_count == 0 || incidence.forward_count == 2)
            })
            .count();
        MeshTopology {
            topological_vertex_count: data.topological_vertex_count,
            edge_count: edges.len(),
            boundary_edge_count,
            non_manifold_edge_count,
            orientation_conflict_edge_count,
            closed: self.vertices.len() >= 4 && self.faces.len() >= 4 && boundary_edge_count == 0,
        }
    }

    /// Returns every exact-location-welded topology edge exactly once.
    ///
    /// This is the curve set Rhino displays and extracts for a triangle mesh:
    /// shared face edges are not duplicated, while naked and non-manifold
    /// edges remain represented.
    pub fn wireframe_lines(&self, tolerance: Tolerance) -> Result<Vec<LineSegment>, GeometryError> {
        let data = self.topology_data();
        data.edges
            .keys()
            .map(|&(first, second)| {
                LineSegment::try_new(
                    data.topological_points[first],
                    data.topological_points[second],
                    tolerance,
                )
            })
            .collect()
    }

    /// Returns each exact-location-welded naked border as a polyline.
    ///
    /// Manifold boundaries are returned as closed, face-oriented loops.
    /// Non-manifold boundary graphs are split deterministically into maximal
    /// trails at branch vertices, with every edge represented exactly once.
    pub fn boundary_polylines(
        &self,
        tolerance: Tolerance,
    ) -> Result<Vec<Polyline3>, GeometryError> {
        let data = self.topology_data();
        let boundary_edges = data
            .edges
            .iter()
            .filter(|(_, incidence)| incidence.count == 1)
            .map(|(&(first, second), incidence)| {
                if incidence
                    .first_use
                    .expect("a boundary edge records its face use")
                    .forward
                {
                    [first, second]
                } else {
                    [second, first]
                }
            })
            .collect::<Vec<_>>();
        if boundary_edges.is_empty() {
            return Ok(Vec::new());
        }

        let mut adjacency = vec![Vec::new(); data.topological_vertex_count];
        for (edge_index, [first, second]) in boundary_edges.iter().copied().enumerate() {
            adjacency[first].push(edge_index);
            adjacency[second].push(edge_index);
        }
        let mut used = vec![false; boundary_edges.len()];
        let mut paths = Vec::new();
        for vertex in 0..adjacency.len() {
            if adjacency[vertex].len() == 2 {
                continue;
            }
            for edge in adjacency[vertex].iter().copied() {
                if !used[edge] {
                    paths.push(trace_boundary_path(
                        vertex,
                        edge,
                        &boundary_edges,
                        &adjacency,
                        &mut used,
                    ));
                }
            }
        }
        for edge in 0..boundary_edges.len() {
            if !used[edge] {
                paths.push(trace_boundary_path(
                    boundary_edges[edge][0],
                    edge,
                    &boundary_edges,
                    &adjacency,
                    &mut used,
                ));
            }
        }

        paths
            .into_iter()
            .map(|path| {
                Polyline3::try_new(
                    path.into_iter()
                        .map(|vertex| data.topological_points[vertex])
                        .collect(),
                    tolerance,
                )
            })
            .collect()
    }

    /// Reorients each manifold-connected face component consistently while
    /// retaining face and vertex order. Exact coincident locations define
    /// adjacency, as they do for [`Self::topology`]. Non-manifold edges do not
    /// impose an ambiguous orientation constraint.
    pub fn unified_face_orientations(&self) -> Result<(Self, usize), GeometryError> {
        let data = self.topology_data();
        let mut neighbors = vec![Vec::<(usize, bool)>::new(); self.faces.len()];
        for incidence in data.edges.values() {
            if incidence.count != 2 {
                continue;
            }
            let first = incidence
                .first_use
                .expect("an edge used twice records its first face");
            let second = incidence
                .second_use
                .expect("an edge used twice records its second face");
            // Equal traversal directions require exactly one adjacent face to
            // flip; opposite directions require both to retain equal parity.
            let opposite_parity = first.forward == second.forward;
            neighbors[first.face].push((second.face, opposite_parity));
            neighbors[second.face].push((first.face, opposite_parity));
        }

        let mut flipped = vec![None; self.faces.len()];
        let mut pending = VecDeque::new();
        for root in 0..self.faces.len() {
            if flipped[root].is_some() {
                continue;
            }
            flipped[root] = Some(false);
            pending.push_back(root);
            while let Some(face) = pending.pop_front() {
                let face_flipped = flipped[face].expect("queued faces have assigned parity");
                for &(neighbor, opposite_parity) in &neighbors[face] {
                    let expected = face_flipped ^ opposite_parity;
                    match flipped[neighbor] {
                        Some(actual) if actual != expected => {
                            return Err(GeometryError::NonOrientableMesh);
                        }
                        Some(_) => {}
                        None => {
                            flipped[neighbor] = Some(expected);
                            pending.push_back(neighbor);
                        }
                    }
                }
            }
        }

        let mut faces = self.faces.clone();
        let mut flipped_face_count = 0;
        for (face, flip) in faces.iter_mut().zip(flipped) {
            if flip.expect("every face component receives an orientation") {
                *face = face.reversed();
                flipped_face_count += 1;
            }
        }
        Ok((
            Self::from_validated_parts(self.vertices.clone(), faces),
            flipped_face_count,
        ))
    }

    /// Splits the mesh into exact-location edge-connected components. A lone
    /// shared vertex does not connect faces. Each result retains source face
    /// order and compacts referenced raw vertices in first-use order.
    pub fn disjoint_pieces(&self) -> Vec<Self> {
        let data = self.topology_data();
        let mut parents = (0..self.faces.len()).collect::<Vec<_>>();
        let mut ranks = vec![0_u8; self.faces.len()];
        for incidence in data.edges.values() {
            let mut uses = incidence.uses();
            let Some(first) = uses.next() else {
                continue;
            };
            for edge_use in uses {
                union_faces(&mut parents, &mut ranks, first.face, edge_use.face);
            }
        }

        let mut component_by_root = BTreeMap::new();
        let mut component_faces = Vec::<Vec<usize>>::new();
        for face in 0..self.faces.len() {
            let root = face_root(&mut parents, face);
            let component_count = component_faces.len();
            let component = *component_by_root.entry(root).or_insert_with(|| {
                component_faces.push(Vec::new());
                component_count
            });
            component_faces[component].push(face);
        }

        component_faces
            .into_iter()
            .map(|faces| self.piece_from_faces(&faces))
            .collect()
    }

    /// Removes faces around exact-location edges used by at least
    /// `minimum_face_count` faces. With `hanging_faces_only`, a qualifying
    /// face must also touch an edge used by exactly one face.
    pub fn extract_non_manifold_faces(
        &self,
        minimum_face_count: usize,
        hanging_faces_only: bool,
    ) -> Result<Option<MeshFaceExtraction>, GeometryError> {
        if minimum_face_count < 3 {
            return Err(GeometryError::InvalidNonManifoldMinimumFaceCount(
                minimum_face_count,
            ));
        }
        let data = self.topology_data();
        let mut touches_qualifying_edge = vec![false; self.faces.len()];
        let mut touches_boundary_edge = vec![false; self.faces.len()];
        for incidence in data.edges.values() {
            if incidence.count >= minimum_face_count {
                for edge_use in incidence.uses() {
                    touches_qualifying_edge[edge_use.face] = true;
                }
            }
            if incidence.count == 1 {
                let edge_use = incidence
                    .first_use
                    .expect("a boundary edge records its incident face");
                touches_boundary_edge[edge_use.face] = true;
            }
        }
        let extracted_mask = touches_qualifying_edge
            .into_iter()
            .zip(touches_boundary_edge)
            .map(|(qualifying, boundary)| qualifying && (!hanging_faces_only || boundary))
            .collect::<Vec<_>>();
        let extracted_faces = extracted_mask
            .iter()
            .enumerate()
            .filter_map(|(face, &extracted)| extracted.then_some(face))
            .collect::<Vec<_>>();
        if extracted_faces.is_empty() {
            return Ok(None);
        }
        let remainder_faces = (0..self.faces.len())
            .filter(|&face| !extracted_mask[face])
            .collect::<Vec<_>>();
        let extracted = self.subset_preserving_vertex_order(&extracted_faces);
        let remainder = (!remainder_faces.is_empty())
            .then(|| self.subset_preserving_vertex_order(&remainder_faces));
        Ok(Some(MeshFaceExtraction {
            remainder,
            extracted,
        }))
    }

    /// Extracts all but one face from every exact-location duplicate class.
    /// Vertex indices, cyclic order, and winding do not affect equality. The
    /// first source face in each class is retained so the result is stable.
    pub fn extract_duplicate_faces(&self) -> Option<MeshFaceExtraction> {
        let mut representative_locations = BTreeSet::new();
        let mut extracted_mask = vec![false; self.faces.len()];
        for (face_index, face) in self.faces.iter().enumerate() {
            let mut locations = face
                .indices()
                .iter()
                .map(|&vertex| {
                    self.vertices[vertex as usize]
                        .to_array()
                        .map(canonical_coordinate_bits)
                })
                .collect::<Vec<_>>();
            locations.sort_unstable();
            if !representative_locations.insert(locations) {
                extracted_mask[face_index] = true;
            }
        }
        let extracted_faces = extracted_mask
            .iter()
            .enumerate()
            .filter_map(|(face, &extracted)| extracted.then_some(face))
            .collect::<Vec<_>>();
        if extracted_faces.is_empty() {
            return None;
        }
        let remainder_faces = (0..self.faces.len())
            .filter(|&face| !extracted_mask[face])
            .collect::<Vec<_>>();
        Some(MeshFaceExtraction {
            remainder: Some(self.subset_preserving_vertex_order(&remainder_faces)),
            extracted: self.subset_preserving_vertex_order(&extracted_faces),
        })
    }

    /// Merges vertices at exactly equal locations, ignoring derived normals
    /// and attributes that this mesh representation does not store. Matching
    /// OpenNURBS behavior, a changed mesh sorts unique vertices by descending
    /// `(x, y, z)` while a mesh with no duplicates remains byte-for-byte equal.
    pub fn combined_identical_vertices(&self) -> (Self, usize) {
        let mut unique_by_location = BTreeMap::new();
        for &vertex in &self.vertices {
            let key = vertex.to_array().map(canonical_coordinate_bits);
            unique_by_location.entry(key).or_insert(vertex);
        }
        let removed = self.vertices.len() - unique_by_location.len();
        if removed == 0 {
            return (self.clone(), 0);
        }

        let mut vertices = unique_by_location.into_values().collect::<Vec<_>>();
        vertices.sort_by(compare_points_descending);
        let index_by_location = vertices
            .iter()
            .enumerate()
            .map(|(index, vertex)| {
                (
                    vertex.to_array().map(canonical_coordinate_bits),
                    u32::try_from(index)
                        .expect("a combined mesh cannot have more vertices than its source"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let faces = self
            .faces
            .iter()
            .copied()
            .map(|face| {
                face.remapped(|source| {
                    let key = self.vertices[source as usize]
                        .to_array()
                        .map(canonical_coordinate_bits);
                    index_by_location[&key]
                })
            })
            .collect();
        (Self::from_validated_parts(vertices, faces), removed)
    }

    /// Removes vertices that are not referenced by any face. Referenced
    /// vertices retain their relative source order, and coincident referenced
    /// vertices remain distinct. Face order and winding are unchanged.
    pub fn culled_unused_vertices(&self) -> (Self, usize) {
        let mut used = vec![false; self.vertices.len()];
        for face in &self.faces {
            for &vertex in face.indices() {
                used[vertex as usize] = true;
            }
        }

        let retained_vertex_count = used.iter().filter(|&&is_used| is_used).count();
        let removed_vertex_count = self.vertices.len() - retained_vertex_count;
        if removed_vertex_count == 0 {
            return (self.clone(), 0);
        }

        let mut vertex_remap = vec![0_u32; self.vertices.len()];
        let mut vertices = Vec::with_capacity(retained_vertex_count);
        for (source, (&point, is_used)) in self.vertices.iter().zip(used).enumerate() {
            if !is_used {
                continue;
            }
            vertex_remap[source] = u32::try_from(vertices.len())
                .expect("a culled mesh cannot have more vertices than its source");
            vertices.push(point);
        }
        let faces = self
            .faces
            .iter()
            .copied()
            .map(|face| face.remapped(|vertex| vertex_remap[vertex as usize]))
            .collect();
        (
            Self::from_validated_parts(vertices, faces),
            removed_vertex_count,
        )
    }

    fn subset_preserving_vertex_order(&self, faces: &[usize]) -> Self {
        let mut used = vec![false; self.vertices.len()];
        for &face in faces {
            for &vertex in self.faces[face].indices() {
                used[vertex as usize] = true;
            }
        }
        let mut vertex_remap = vec![0_u32; self.vertices.len()];
        let mut vertices = Vec::new();
        for (source, (&point, used)) in self.vertices.iter().zip(used).enumerate() {
            if !used {
                continue;
            }
            vertex_remap[source] = u32::try_from(vertices.len())
                .expect("a mesh subset cannot have more vertices than its source");
            vertices.push(point);
        }
        let retained_faces = faces
            .iter()
            .map(|&face| self.faces[face].remapped(|vertex| vertex_remap[vertex as usize]))
            .collect();
        Self::from_validated_parts(vertices, retained_faces)
    }

    fn piece_from_faces(&self, faces: &[usize]) -> Self {
        let mut vertex_remap = vec![None; self.vertices.len()];
        let mut vertices = Vec::new();
        let mut retained_faces = Vec::with_capacity(faces.len());
        for &face in faces {
            let retained_face = self.faces[face].remapped(|source| {
                let source = source as usize;
                *vertex_remap[source].get_or_insert_with(|| {
                    let target = u32::try_from(vertices.len())
                        .expect("a mesh component cannot have more vertices than its source");
                    vertices.push(self.vertices[source]);
                    target
                })
            });
            retained_faces.push(retained_face);
        }
        Self::from_validated_parts(vertices, retained_faces)
    }

    fn topology_data(&self) -> MeshTopologyData {
        let mut locations = BTreeMap::<[u64; 3], usize>::new();
        let mut topological_points = Vec::new();
        let mut topological_vertices = Vec::with_capacity(self.vertices.len());
        for vertex in &self.vertices {
            let key = vertex.to_array().map(canonical_coordinate_bits);
            let id = *locations.entry(key).or_insert_with(|| {
                let id = topological_points.len();
                topological_points.push(*vertex);
                id
            });
            topological_vertices.push(id);
        }

        let mut edges = BTreeMap::<(usize, usize), EdgeIncidence>::new();
        for (face_index, face) in self.faces.iter().enumerate() {
            let indices = face.indices();
            for edge in 0..indices.len() {
                let from = topological_vertices[indices[edge] as usize];
                let to = topological_vertices[indices[(edge + 1) % indices.len()] as usize];
                debug_assert_ne!(from, to, "validated mesh edge collapsed");
                let (edge, forward) = if from < to {
                    ((from, to), true)
                } else {
                    ((to, from), false)
                };
                let incidence = edges.entry(edge).or_default();
                incidence.add_use(EdgeUse {
                    face: face_index,
                    forward,
                });
            }
        }

        MeshTopologyData {
            topological_vertex_count: locations.len(),
            topological_points,
            edges,
        }
    }

    pub fn face_normal(&self, index: usize) -> Result<UnitVector3, GeometryError> {
        let points = self
            .triangle_points(index)
            .ok_or(GeometryError::TriangleIndexOutOfRange { triangle: index })?;
        let first = points[0].vector_to(points[1])?.normalized_nonzero()?;
        let second = points[0].vector_to(points[2])?.normalized_nonzero()?;
        first
            .as_vector()
            .cross(second.as_vector())?
            .normalized_nonzero()
    }

    pub fn area(&self) -> Result<Real, GeometryError> {
        let mut sum = 0.0;
        let mut correction = 0.0;
        for index in 0..self.triangles.len() {
            let points = self
                .triangle_points(index)
                .expect("a validated mesh has valid triangle indices");
            let first = points[0].vector_to(points[1])?;
            let second = points[0].vector_to(points[2])?;
            let area = first.cross(second)?.length()? * 0.5;
            require_finite([area], "mesh face area")?;
            let next = sum + area;
            if sum.abs() >= area.abs() {
                correction += (sum - next) + area;
            } else {
                correction += (area - next) + sum;
            }
            sum = next;
        }
        let area = sum + correction;
        require_finite([area], "mesh area")?;
        Ok(area)
    }

    /// Computes oriented mesh volume. Outward winding is positive and
    /// reversing every face negates the result. A bounding-box-center base
    /// point and normalized coordinates keep large translations from
    /// introducing cancellation or intermediate overflow.
    pub fn signed_volume(&self) -> Result<Real, GeometryError> {
        let mut used = vec![false; self.vertices.len()];
        for triangle in &self.triangles {
            for vertex in triangle {
                used[*vertex as usize] = true;
            }
        }
        let base = BoundingBox3::from_points(
            self.vertices
                .iter()
                .zip(&used)
                .filter_map(|(&point, &used)| used.then_some(point)),
        )?
        .center()?;
        let mut relative_vertices = vec![[0.0; 3]; self.vertices.len()];
        let mut scale: Real = 0.0;
        for (index, (&vertex, &used)) in self.vertices.iter().zip(&used).enumerate() {
            if !used {
                continue;
            }
            let relative = base.vector_to(vertex)?.to_array();
            scale = relative
                .iter()
                .fold(scale, |current, coordinate| current.max(coordinate.abs()));
            relative_vertices[index] = relative;
        }
        debug_assert!(scale > 0.0, "a validated mesh has non-coincident vertices");
        for (relative, used) in relative_vertices.iter_mut().zip(used) {
            if !used {
                continue;
            }
            for coordinate in relative.iter_mut() {
                *coordinate /= scale;
            }
        }

        let mut sum = 0.0;
        let mut correction = 0.0;
        for triangle in &self.triangles {
            let a = relative_vertices[triangle[0] as usize];
            let b = relative_vertices[triangle[1] as usize];
            let c = relative_vertices[triangle[2] as usize];
            let cross = [
                b[1].mul_add(c[2], -b[2] * c[1]),
                b[2].mul_add(c[0], -b[0] * c[2]),
                b[0].mul_add(c[1], -b[1] * c[0]),
            ];
            let determinant = a[0].mul_add(cross[0], a[1].mul_add(cross[1], a[2] * cross[2]));
            let next = sum + determinant;
            if sum.abs() >= determinant.abs() {
                correction += (sum - next) + determinant;
            } else {
                correction += (determinant - next) + sum;
            }
            sum = next;
        }
        let normalized_volume = (sum + correction) / 6.0;
        require_finite([normalized_volume], "mesh volume")?;
        if normalized_volume == 0.0 {
            return Ok(0.0);
        }
        let scaled_square = product_three(normalized_volume.abs(), scale, scale, "mesh volume")?;
        let magnitude = scaled_square * scale;
        require_finite([magnitude], "mesh volume")?;
        Ok(normalized_volume.signum() * magnitude)
    }

    pub fn bounds(&self) -> BoundingBox3 {
        BoundingBox3::from_points(self.vertices.iter().copied())
            .expect("a validated mesh has triangle vertices")
    }

    pub fn transformed(
        &self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let vertices = self
            .vertices
            .iter()
            .map(|point| transform.transform_point(*point))
            .collect::<Result<_, _>>()?;
        Self::try_new_faces(vertices, self.faces.clone(), tolerance)
    }
}

fn validate_triangle(
    vertices: &[Point3],
    triangle: [u32; 3],
    face_index: usize,
    from_quad: bool,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let point_at = |vertex_index| {
        vertices.get(vertex_index as usize).copied().ok_or({
            if from_quad {
                GeometryError::InvalidQuadIndex {
                    face: face_index,
                    vertex: vertex_index,
                }
            } else {
                GeometryError::InvalidTriangleIndex {
                    triangle: face_index,
                    vertex: vertex_index,
                }
            }
        })
    };
    let points = [
        point_at(triangle[0])?,
        point_at(triangle[1])?,
        point_at(triangle[2])?,
    ];
    let degenerate = || {
        if from_quad {
            GeometryError::DegenerateQuad { face: face_index }
        } else {
            GeometryError::DegenerateTriangle {
                triangle: face_index,
            }
        }
    };
    let first_edge = points[0]
        .vector_to(points[1])?
        .normalized(tolerance)
        .map_err(|_| degenerate())?;
    let second_edge = points[0]
        .vector_to(points[2])?
        .normalized(tolerance)
        .map_err(|_| degenerate())?;
    let sine = first_edge
        .as_vector()
        .cross(second_edge.as_vector())?
        .length()?;
    if sine <= tolerance.angular() {
        return Err(degenerate());
    }
    Ok(())
}

fn trace_boundary_path(
    start: usize,
    first_edge: usize,
    edges: &[[usize; 2]],
    adjacency: &[Vec<usize>],
    used: &mut [bool],
) -> Vec<usize> {
    let mut path = vec![start];
    let mut current = start;
    let mut edge = first_edge;
    loop {
        debug_assert!(!used[edge]);
        used[edge] = true;
        let [first, second] = edges[edge];
        let next = if current == first {
            second
        } else {
            debug_assert_eq!(current, second);
            first
        };
        path.push(next);
        if next == start || adjacency[next].len() != 2 {
            break;
        }
        let Some(next_edge) = adjacency[next]
            .iter()
            .copied()
            .find(|candidate| !used[*candidate])
        else {
            break;
        };
        current = next;
        edge = next_edge;
    }
    path
}

fn canonical_coordinate_bits(coordinate: Real) -> u64 {
    if coordinate == 0.0 {
        0
    } else {
        coordinate.to_bits()
    }
}

fn compare_points_descending(left: &Point3, right: &Point3) -> std::cmp::Ordering {
    left.to_array()
        .into_iter()
        .zip(right.to_array())
        .map(|(left, right)| {
            right
                .partial_cmp(&left)
                .expect("validated point coordinates are finite")
        })
        .find(|ordering| !ordering.is_eq())
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn face_root(parents: &mut [usize], face: usize) -> usize {
    let mut root = face;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = face;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn union_faces(parents: &mut [usize], ranks: &mut [u8], first: usize, second: usize) {
    let first_root = face_root(parents, first);
    let second_root = face_root(parents, second);
    if first_root == second_root {
        return;
    }
    match ranks[first_root].cmp(&ranks[second_root]) {
        std::cmp::Ordering::Less => parents[first_root] = second_root,
        std::cmp::Ordering::Greater => parents[second_root] = first_root,
        std::cmp::Ordering::Equal => {
            parents[second_root] = first_root;
            ranks[first_root] += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn validates_indices_degeneracy_and_orientation() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
        ];
        let mesh =
            TriangleMesh::try_new(vertices.clone(), vec![[0, 1, 2]], Tolerance::DEFAULT).unwrap();
        assert_eq!(mesh.face_normal(0).unwrap().z(), 1.0);
        assert_eq!(mesh.bounds().max(), point(1.0, 1.0, 0.0));
        assert!(matches!(
            mesh.face_normal(1),
            Err(GeometryError::TriangleIndexOutOfRange { triangle: 1 })
        ));

        assert!(
            TriangleMesh::try_new(vertices.clone(), vec![[0, 1, 3]], Tolerance::DEFAULT).is_err()
        );
        assert!(TriangleMesh::try_new(vertices, vec![[0, 1, 1]], Tolerance::DEFAULT).is_err());
        assert!(matches!(
            TriangleMesh::try_new(Vec::new(), vec![[0, 1, 2]], Tolerance::DEFAULT),
            Err(GeometryError::InvalidTriangleIndex {
                triangle: 0,
                vertex: 0
            })
        ));

        let square = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(square.area().unwrap(), 1.0);
    }

    #[test]
    fn preserves_quadrilateral_faces_without_topology_diagonals() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 3.0, 0.0),
            point(0.0, 3.0, 0.0),
        ];
        let mesh = TriangleMesh::try_new_faces(
            vertices.clone(),
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(mesh.faces(), &[MeshFace::Quad([0, 1, 2, 3])]);
        assert_eq!(mesh.triangles(), &[[0, 1, 2], [0, 2, 3]]);
        assert_eq!(mesh.topology().edge_count(), 4);
        assert_eq!(mesh.wireframe_lines(Tolerance::DEFAULT).unwrap().len(), 4);
        assert_eq!(mesh.area().unwrap(), 6.0);
        assert_eq!(mesh.reversed().faces(), &[MeshFace::Quad([0, 3, 2, 1])]);

        assert!(matches!(
            TriangleMesh::try_new_faces(
                vertices.clone(),
                vec![MeshFace::Quad([0, 1, 2, 4])],
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidQuadIndex { face: 0, vertex: 4 })
        ));
        assert!(matches!(
            TriangleMesh::try_new_faces(
                vertices,
                vec![MeshFace::Quad([0, 1, 2, 1])],
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::DegenerateQuad { face: 0 })
        ));
    }

    #[test]
    fn computes_closed_quad_mesh_topology_and_volume() {
        let cube = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
                point(1.0, 0.0, 1.0),
                point(1.0, 1.0, 1.0),
                point(0.0, 1.0, 1.0),
            ],
            vec![
                MeshFace::Quad([0, 3, 2, 1]),
                MeshFace::Quad([4, 5, 6, 7]),
                MeshFace::Quad([0, 1, 5, 4]),
                MeshFace::Quad([1, 2, 6, 5]),
                MeshFace::Quad([2, 3, 7, 6]),
                MeshFace::Quad([3, 0, 4, 7]),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(cube.face_count(), 6);
        assert_eq!(cube.triangles().len(), 12);
        assert_eq!(cube.topology().edge_count(), 12);
        assert!(cube.topology().is_solid());
        assert!((cube.signed_volume().unwrap() - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn topology_detects_closed_oriented_and_open_meshes() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ];
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let tetrahedron =
            TriangleMesh::try_new(vertices, faces.clone(), Tolerance::DEFAULT).unwrap();
        let topology = tetrahedron.topology();
        assert_eq!(topology.topological_vertex_count(), 4);
        assert_eq!(topology.edge_count(), 6);
        assert_eq!(topology.boundary_edge_count(), 0);
        assert_eq!(topology.non_manifold_edge_count(), 0);
        assert_eq!(topology.orientation_conflict_edge_count(), 0);
        assert!(topology.is_closed());
        assert!(topology.is_manifold());
        assert!(topology.is_oriented());
        assert!(topology.is_solid());

        let open = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .topology();
        assert_eq!(open.boundary_edge_count(), 3);
        assert!(!open.is_closed());
        assert!(open.is_manifold());
        assert!(open.is_oriented());

        let mut flipped_faces = faces;
        flipped_faces[0].swap(1, 2);
        let unoriented = TriangleMesh::try_new(
            tetrahedron.vertices().to_vec(),
            flipped_faces,
            Tolerance::DEFAULT,
        )
        .unwrap()
        .topology();
        assert!(unoriented.is_closed());
        assert!(unoriented.is_manifold());
        assert!(!unoriented.is_oriented());
        assert_eq!(unoriented.orientation_conflict_edge_count(), 3);
        assert!(!unoriented.is_solid());
    }

    #[test]
    fn extracts_welded_mesh_boundary_loops_without_internal_edges() {
        let square_soup = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 2.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let borders = square_soup.boundary_polylines(Tolerance::DEFAULT).unwrap();
        assert_eq!(borders.len(), 1);
        assert!(borders[0].is_closed());
        assert_eq!(borders[0].segment_count(), 4);
        assert!((borders[0].length().unwrap() - 8.0).abs() < 1.0e-12);

        let ring = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(3.0, 3.0, 0.0),
                point(1.0, 3.0, 0.0),
            ],
            vec![
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut lengths = ring
            .boundary_polylines(Tolerance::DEFAULT)
            .unwrap()
            .into_iter()
            .map(|border| {
                assert!(border.is_closed());
                border.length().unwrap()
            })
            .collect::<Vec<_>>();
        lengths.sort_by(Real::total_cmp);
        assert_eq!(lengths, [8.0, 16.0]);

        let closed = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            closed
                .boundary_polylines(Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn wireframe_returns_each_welded_topology_edge_once() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 2.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let lines = mesh.wireframe_lines(Tolerance::DEFAULT).unwrap();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines.len(), mesh.topology().edge_count());
        assert_eq!(
            lines
                .iter()
                .filter(|line| {
                    [line.start(), line.end()].contains(&point(0.0, 0.0, 0.0))
                        && [line.start(), line.end()].contains(&point(2.0, 2.0, 0.0))
                })
                .count(),
            1
        );
    }

    #[test]
    fn computes_translation_stable_oriented_volume_and_reports_overflow() {
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let tetrahedron = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 4.0),
            ],
            faces.clone(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(tetrahedron.signed_volume().unwrap(), 4.0);
        assert_eq!(tetrahedron.reversed().signed_volume().unwrap(), -4.0);

        let translated = TriangleMesh::try_new(
            vec![
                point(1.0e9, -2.0e9, 3.0e9),
                point(1.0e9 + 2.0, -2.0e9, 3.0e9),
                point(1.0e9, -2.0e9 + 3.0, 3.0e9),
                point(1.0e9, -2.0e9, 3.0e9 + 4.0),
                point(1.0e200, -1.0e200, 1.0e200),
            ],
            faces.clone(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(translated.signed_volume().unwrap(), 4.0);

        let overflowing = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0e110, 0.0, 0.0),
                point(0.0, 3.0e110, 0.0),
                point(0.0, 0.0, 4.0e110),
            ],
            faces,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            overflowing.signed_volume(),
            Err(GeometryError::NonFinite {
                context: "mesh volume"
            })
        );
    }

    #[test]
    fn topology_welds_triangle_soup_and_reports_non_manifold_edges() {
        let locations = [
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ];
        let oriented_faces = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let mut soup_vertices = Vec::new();
        let mut soup_faces = Vec::new();
        for face in oriented_faces {
            let start = soup_vertices.len() as u32;
            soup_vertices.extend(face.map(|index| locations[index]));
            soup_faces.push([start, start + 1, start + 2]);
        }
        let soup = TriangleMesh::try_new(soup_vertices, soup_faces, Tolerance::DEFAULT).unwrap();
        let topology = soup.topology();
        assert_eq!(soup.vertices().len(), 12);
        assert_eq!(topology.topological_vertex_count(), 4);
        assert!(topology.is_solid());

        let vertices = vec![
            locations[0],
            locations[1],
            locations[2],
            locations[3],
            point(0.0, 0.0, -1.0),
        ];
        let two_tetrahedra = TriangleMesh::try_new(
            vertices,
            vec![
                [0, 2, 1],
                [0, 1, 3],
                [0, 3, 2],
                [1, 2, 3],
                [0, 1, 2],
                [0, 4, 1],
                [0, 2, 4],
                [1, 4, 2],
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .topology();
        assert!(two_tetrahedra.is_closed());
        assert_eq!(two_tetrahedra.non_manifold_edge_count(), 3);
        assert!(!two_tetrahedra.is_manifold());
        assert!(!two_tetrahedra.is_oriented());
        assert!(!two_tetrahedra.is_solid());
    }

    #[test]
    fn reverses_and_unifies_face_winding_without_reordering_faces() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ];
        let oriented_faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let oriented =
            TriangleMesh::try_new(vertices.clone(), oriented_faces.clone(), Tolerance::DEFAULT)
                .unwrap();
        let reversed = oriented.reversed();
        assert_eq!(reversed.reversed(), oriented);
        for index in 0..oriented.triangles().len() {
            let normal = oriented.face_normal(index).unwrap();
            let reversed_normal = reversed.face_normal(index).unwrap();
            let dot = normal.as_vector().dot(reversed_normal.as_vector()).unwrap();
            assert!((dot + 1.0).abs() <= 2.0 * Real::EPSILON);
        }

        let mut inconsistent_faces = oriented_faces.clone();
        inconsistent_faces[1].swap(1, 2);
        let inconsistent =
            TriangleMesh::try_new(vertices, inconsistent_faces, Tolerance::DEFAULT).unwrap();
        assert_eq!(inconsistent.topology().orientation_conflict_edge_count(), 3);
        let (unified, flipped_face_count) = inconsistent.unified_face_orientations().unwrap();
        assert_eq!(flipped_face_count, 1);
        assert_eq!(unified.triangles(), oriented_faces);
        assert!(unified.topology().is_oriented());
        assert_eq!(inconsistent.triangles()[1], [0, 3, 1]);
    }

    #[test]
    fn unification_uses_exact_locations_and_rejects_non_orientable_topology() {
        let locations = [
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ];
        let mut soup_vertices = Vec::new();
        let mut soup_faces = Vec::new();
        for (face_index, mut face) in [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]
            .into_iter()
            .enumerate()
        {
            if face_index == 2 {
                face.swap(1, 2);
            }
            let start = soup_vertices.len() as u32;
            soup_vertices.extend(face.map(|index| locations[index]));
            soup_faces.push([start, start + 1, start + 2]);
        }
        let soup = TriangleMesh::try_new(soup_vertices, soup_faces, Tolerance::DEFAULT).unwrap();
        let (unified, flipped_face_count) = soup.unified_face_orientations().unwrap();
        assert_eq!(flipped_face_count, 1);
        assert!(unified.topology().is_oriented());

        const SEGMENTS: usize = 4;
        let mut vertices = Vec::with_capacity(SEGMENTS * 2);
        for index in 0..SEGMENTS {
            let angle = std::f64::consts::TAU * index as Real / SEGMENTS as Real;
            for lateral in [-0.25, 0.25] {
                let radius = 2.0 + lateral * (angle * 0.5).cos();
                vertices.push(point(
                    radius * angle.cos(),
                    radius * angle.sin(),
                    lateral * (angle * 0.5).sin(),
                ));
            }
        }
        let mut faces = Vec::with_capacity(SEGMENTS * 2);
        for index in 0..SEGMENTS {
            let a = (index * 2) as u32;
            let b = a + 1;
            let (next_a, next_b) = if index + 1 == SEGMENTS {
                (1, 0)
            } else {
                (a + 2, b + 2)
            };
            faces.push([a, next_a, b]);
            faces.push([next_a, next_b, b]);
        }
        let mobius = TriangleMesh::try_new(vertices, faces, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            mobius.unified_face_orientations(),
            Err(GeometryError::NonOrientableMesh)
        );
    }

    #[test]
    fn splits_edge_connected_components_and_compacts_vertices_in_source_order() {
        let source = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(11.0, 0.0, 0.0),
                point(10.0, 1.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [4, 5, 6], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let pieces = source.disjoint_pieces();
        assert_eq!(pieces.len(), 2);
        assert_eq!(
            pieces[0].vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
            ]
        );
        assert_eq!(pieces[0].triangles(), &[[0, 1, 2], [0, 2, 3]]);
        assert_eq!(
            pieces[1].vertices(),
            &[
                point(10.0, 0.0, 0.0),
                point(11.0, 0.0, 0.0),
                point(10.0, 1.0, 0.0),
            ]
        );
        assert_eq!(pieces[1].triangles(), &[[0, 1, 2]]);
    }

    #[test]
    fn disjoint_pieces_require_an_edge_but_accept_exact_duplicate_edge_locations() {
        let vertex_touch = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(-1.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 3, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let pieces = vertex_touch.disjoint_pieces();
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].vertices()[0], pieces[1].vertices()[0]);

        let duplicate_edge = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let pieces = duplicate_edge.disjoint_pieces();
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0], duplicate_edge);
    }

    #[test]
    fn extracts_all_or_only_hanging_faces_from_non_manifold_edges() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
            point(0.0, -1.0, 1.0),
        ];
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3], [0, 1, 4]];
        let mesh =
            TriangleMesh::try_new(vertices.clone(), faces.clone(), Tolerance::DEFAULT).unwrap();
        assert_eq!(mesh.topology().non_manifold_edge_count(), 1);

        let all = mesh.extract_non_manifold_faces(3, false).unwrap().unwrap();
        assert_eq!(all.extracted().vertices(), vertices);
        assert_eq!(all.extracted().triangles(), &[faces[0], faces[1], faces[4]]);
        let remainder = all.remainder().unwrap();
        assert_eq!(remainder.vertices(), &vertices[..4]);
        assert_eq!(remainder.triangles(), &[faces[2], faces[3]]);

        let hanging = mesh.extract_non_manifold_faces(3, true).unwrap().unwrap();
        assert_eq!(
            hanging.extracted().vertices(),
            &[vertices[0], vertices[1], vertices[4]]
        );
        assert_eq!(hanging.extracted().triangles(), &[[0, 1, 2]]);
        assert_eq!(hanging.remainder().unwrap().vertices(), &vertices[..4]);
        assert_eq!(hanging.remainder().unwrap().triangles(), &faces[..4]);

        assert!(mesh.extract_non_manifold_faces(4, false).unwrap().is_none());
        assert_eq!(
            mesh.extract_non_manifold_faces(2, false),
            Err(GeometryError::InvalidNonManifoldMinimumFaceCount(2))
        );
    }

    #[test]
    fn extracts_exact_location_duplicate_faces_independent_of_winding() {
        let points = [
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(-0.0, 0.0, 0.0),
            point(2.0, -0.0, 0.0),
            point(0.0, 2.0, -0.0),
            point(0.0, 0.0, 1.0e-12),
            point(2.0, 0.0, 1.0e-12),
            point(0.0, 2.0, 1.0e-12),
        ];
        let faces = vec![[0, 1, 2], [1, 2, 0], [3, 5, 4], [6, 7, 8]];
        let mesh =
            TriangleMesh::try_new(points.to_vec(), faces.clone(), Tolerance::DEFAULT).unwrap();
        let extraction = mesh.extract_duplicate_faces().unwrap();

        assert_eq!(extraction.extracted().triangles(), &[[1, 2, 0], [3, 5, 4]]);
        assert_eq!(extraction.extracted().vertices(), &points[..6]);
        let remainder = extraction.remainder().unwrap();
        assert_eq!(remainder.triangles(), &[[0, 1, 2], [3, 4, 5]]);
        assert_eq!(
            remainder.vertices(),
            &[
                points[0], points[1], points[2], points[6], points[7], points[8]
            ]
        );

        let signed_zero_duplicate = TriangleMesh::try_new(
            points[..6].to_vec(),
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(signed_zero_duplicate.extract_duplicate_faces().is_some());
        let truly_unique = TriangleMesh::try_new(
            points.to_vec(),
            vec![[0, 1, 2], [6, 7, 8]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(truly_unique.extract_duplicate_faces().is_none());
    }

    #[test]
    fn combines_identical_vertices_in_rhino_order_without_culling_unused() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(2.0, -0.0, 0.0),
            point(-0.0, 0.0, 0.0),
            point(0.0, -2.0, 0.0),
            point(0.0, 2.0, -0.0),
            point(99.0, 99.0, 99.0),
        ];
        let mesh = TriangleMesh::try_new(vertices, vec![[0, 1, 2], [3, 4, 5]], Tolerance::DEFAULT)
            .unwrap();
        let (combined, removed) = mesh.combined_identical_vertices();
        assert_eq!(removed, 3);
        assert_eq!(
            combined.vertices(),
            &[
                point(99.0, 99.0, 99.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
            ]
        );
        assert_eq!(combined.triangles(), &[[3, 1, 2], [1, 3, 4]]);

        let unique = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(unique.combined_identical_vertices(), (unique.clone(), 0));

        let near = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 1.0e-12),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(near.combined_identical_vertices(), (near.clone(), 0));
    }

    #[test]
    fn culls_only_unused_vertices_in_source_order() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(99.0, 99.0, 99.0),
                point(0.0, 0.0, 0.0),
                point(98.0, 98.0, 98.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(97.0, 97.0, 97.0),
            ],
            vec![[1, 3, 4], [3, 5, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();

        let (culled, removed) = mesh.culled_unused_vertices();
        assert_eq!(removed, 3);
        assert_eq!(
            culled.vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
            ]
        );
        assert_eq!(culled.triangles(), &[[0, 1, 2], [1, 3, 2]]);

        let already_compact = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            already_compact.culled_unused_vertices(),
            (already_compact.clone(), 0)
        );
    }

    #[test]
    fn derived_normals_do_not_depend_on_a_new_model_tolerance() {
        let tolerance = Tolerance::try_new(1.0e-15, 1.0e-15, 1.0e-15).unwrap();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0e-12, 0.0, 0.0),
                point(0.0, 1.0e-12, 0.0),
            ],
            vec![[0, 1, 2]],
            tolerance,
        )
        .unwrap();

        assert_eq!(mesh.face_normal(0).unwrap().z(), 1.0);
    }

    #[test]
    fn transforms_vertices_and_rejects_collapsed_faces() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let moved = mesh
            .transformed(
                AffineTransform3::from_translation(crate::Vector3::try_new(2.0, 3.0, 4.0).unwrap()),
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(moved.vertices()[0], point(2.0, 3.0, 4.0));
        assert_eq!(moved.vertices()[2], point(2.0, 4.0, 4.0));

        let collapsed = AffineTransform3::try_new(
            [[1.0, 0.0, 0.0], [0.0; 3], [0.0; 3]],
            crate::Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        assert!(mesh.transformed(collapsed, Tolerance::DEFAULT).is_err());
    }
}
