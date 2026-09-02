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

/// Selects topology edges for mesh-curve extraction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshEdgeFilter {
    /// Edges used by exactly one polygon face.
    Naked,
    /// Naked edges and coincident seams whose faces use distinct raw vertices.
    Unwelded,
    /// Edges whose greatest incident-face normal angle lies strictly inside
    /// this radian interval.
    FaceAngle {
        greater_than_radians: Real,
        less_than_radians: Real,
    },
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
    side: usize,
    forward: bool,
    /// Raw mesh vertex indices ordered like the canonical topology edge.
    raw_vertices: [u32; 2],
}

#[derive(Debug)]
struct MeshTopologyData {
    topological_vertex_count: usize,
    topological_points: Vec<Point3>,
    topological_vertices: Vec<usize>,
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

    /// Returns selected exact-location topology edges as canonical lines.
    ///
    /// [`MeshEdgeFilter::Unwelded`] follows Rhino/OpenNURBS terminology: a
    /// naked edge is included, and an interior seam is included only when all
    /// incident faces use distinct raw vertex indices at both endpoints.
    pub fn filtered_edge_lines(
        &self,
        filter: MeshEdgeFilter,
        tolerance: Tolerance,
    ) -> Result<Vec<LineSegment>, GeometryError> {
        validate_mesh_edge_filter(filter)?;
        let data = self.topology_data();
        let face_normals = matches!(filter, MeshEdgeFilter::FaceAngle { .. })
            .then(|| self.polygon_face_normals())
            .transpose()?;
        data.edges
            .iter()
            .filter(|(_, incidence)| {
                mesh_edge_matches_filter(incidence, filter, face_normals.as_deref())
            })
            .map(|(&(first, second), _)| {
                LineSegment::try_new(
                    data.topological_points[first],
                    data.topological_points[second],
                    tolerance,
                )
            })
            .collect()
    }

    /// Joins selected topology edges into deterministic, edge-exact trails.
    ///
    /// Each selected edge appears once. An Euler trail is used when a
    /// connected network has zero or two odd vertices; more highly branched
    /// networks are decomposed into the minimum number of open trails by
    /// temporarily pairing their odd vertices.
    pub fn filtered_edge_polylines(
        &self,
        filter: MeshEdgeFilter,
        tolerance: Tolerance,
    ) -> Result<Vec<Polyline3>, GeometryError> {
        validate_mesh_edge_filter(filter)?;
        let data = self.topology_data();
        let face_normals = matches!(filter, MeshEdgeFilter::FaceAngle { .. })
            .then(|| self.polygon_face_normals())
            .transpose()?;
        let edges = data
            .edges
            .iter()
            .filter_map(|(&(first, second), incidence)| {
                mesh_edge_matches_filter(incidence, filter, face_normals.as_deref())
                    .then_some([first, second])
            })
            .collect::<Vec<_>>();
        topology_edge_polylines(&data.topological_points, &edges, tolerance)
    }

    /// Returns the logical mesh edges induced by a face break angle.
    ///
    /// Naked and unwelded topology edges always form boundaries. A welded
    /// interior edge forms a boundary when at least two of its incident face
    /// normals meet at or above `break_angle_radians`; angles strictly below
    /// the threshold are treated as one smooth face region. At each topology
    /// vertex, boundary segments join only when the locally adjacent smooth
    /// face regions agree. This keeps a crease separate from an intersecting
    /// crease while allowing a tessellated open border to become one polyline.
    pub fn logical_edge_polylines(
        &self,
        break_angle_radians: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<Polyline3>, GeometryError> {
        validate_mesh_break_angle(break_angle_radians)?;
        let data = self.topology_data();
        let face_normals = self.polygon_face_normals()?;
        let edge_records = data
            .edges
            .iter()
            .map(|(&(first, second), incidence)| ([first, second], incidence))
            .collect::<Vec<_>>();
        let boundary_edges = edge_records
            .iter()
            .map(|(_, incidence)| {
                mesh_edge_is_logical_boundary(incidence, &face_normals, break_angle_radians)
            })
            .collect::<Vec<_>>();
        if !boundary_edges.iter().any(|&boundary| boundary) {
            return Ok(Vec::new());
        }

        let mut incident_edges = vec![Vec::new(); data.topological_vertex_count];
        for (edge, (vertices, _)) in edge_records.iter().enumerate() {
            incident_edges[vertices[0]].push(edge);
            incident_edges[vertices[1]].push(edge);
        }
        let mut endpoint_signatures = vec![[Vec::new(), Vec::new()]; edge_records.len()];
        for (vertex, incident) in incident_edges.iter().enumerate() {
            let mut faces = incident
                .iter()
                .flat_map(|&edge| edge_records[edge].1.uses().map(|edge_use| edge_use.face))
                .collect::<Vec<_>>();
            faces.sort_unstable();
            faces.dedup();
            let mut parents = (0..faces.len()).collect::<Vec<_>>();
            let mut ranks = vec![0_u8; faces.len()];
            for &edge in incident {
                if boundary_edges[edge] {
                    continue;
                }
                let mut uses = edge_records[edge].1.uses();
                let Some(first) = uses.next() else {
                    continue;
                };
                let first = faces
                    .binary_search(&first.face)
                    .expect("an incident edge face is present at its vertex");
                for edge_use in uses {
                    let face = faces
                        .binary_search(&edge_use.face)
                        .expect("an incident edge face is present at its vertex");
                    union_faces(&mut parents, &mut ranks, first, face);
                }
            }

            for &edge in incident {
                if !boundary_edges[edge] {
                    continue;
                }
                let mut signature = edge_records[edge]
                    .1
                    .uses()
                    .map(|edge_use| {
                        let face = faces
                            .binary_search(&edge_use.face)
                            .expect("an incident edge face is present at its vertex");
                        face_root(&mut parents, face)
                    })
                    .collect::<Vec<_>>();
                if edge_records[edge].1.count == 1 {
                    // The exterior is a distinct local region after every real
                    // incident face. Its stable sentinel lets adjacent naked
                    // edges join without colliding with a face-region root.
                    signature.push(faces.len());
                }
                signature.sort_unstable();
                let endpoint = usize::from(edge_records[edge].0[1] == vertex);
                endpoint_signatures[edge][endpoint] = signature;
            }
        }

        let mut logical_vertex_indices = BTreeMap::<(usize, Vec<usize>), usize>::new();
        let mut logical_points = Vec::new();
        let mut logical_edges = Vec::new();
        for (edge, ((vertices, _), boundary)) in edge_records.iter().zip(boundary_edges).enumerate()
        {
            if !boundary {
                continue;
            }
            let mut logical_vertices = [0_usize; 2];
            for endpoint in 0..2 {
                let key = (
                    vertices[endpoint],
                    endpoint_signatures[edge][endpoint].clone(),
                );
                logical_vertices[endpoint] = if let Some(&index) = logical_vertex_indices.get(&key)
                {
                    index
                } else {
                    let index = logical_points.len();
                    logical_points.push(data.topological_points[vertices[endpoint]]);
                    logical_vertex_indices.insert(key, index);
                    index
                };
            }
            logical_edges.push(logical_vertices);
        }
        topology_edge_polylines(&logical_points, &logical_edges, tolerance)
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

    /// Splits the mesh into the parts Rhino's `Explode` command sees across
    /// unwelded edges.
    ///
    /// Exact coincident positions establish topological edges. Such an edge
    /// remains welded when its incident faces reuse a raw vertex index at
    /// either endpoint; it is unwelded only when every incident use has a
    /// distinct raw index at both endpoints. A lone shared vertex never joins
    /// parts. Results retain source face order and preserve logical quads.
    pub fn explode_pieces(&self) -> Vec<Self> {
        let data = self.topology_data();
        let mut parents = (0..self.faces.len()).collect::<Vec<_>>();
        let mut ranks = vec![0_u8; self.faces.len()];
        for incidence in data.edges.values() {
            let uses = incidence.uses().collect::<Vec<_>>();
            if uses.len() == 1 || edge_uses_are_unwelded(&uses) {
                continue;
            }
            for edge_use in &uses[1..] {
                union_faces(&mut parents, &mut ranks, uses[0].face, edge_use.face);
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

    /// Welds coincident edge endpoints whose incident face normals fall
    /// within the supplied angular tolerance.
    ///
    /// Only vertices paired along an exact-location topology edge can merge;
    /// a coincident vertex-only contact remains distinct. The later raw vertex
    /// is retained as Rhino/OpenNURBS does, then all unreferenced vertices are
    /// compacted while the remaining source order is preserved.
    pub fn welded_vertices(
        &self,
        angle_tolerance_radians: Real,
    ) -> Result<(Self, usize), GeometryError> {
        if !(angle_tolerance_radians.is_finite()
            && (0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians))
        {
            return Err(GeometryError::InvalidMeshWeldAngle);
        }
        let data = self.topology_data();
        let face_normals = self.polygon_face_normals()?;
        let minimum_dot = angle_tolerance_radians.cos();
        let mut parents = (0..self.vertices.len()).collect::<Vec<_>>();
        for incidence in data.edges.values() {
            let uses = incidence.uses().collect::<Vec<_>>();
            for left in 0..uses.len().saturating_sub(1) {
                for right in left + 1..uses.len() {
                    let dot = face_normals[uses[left].face]
                        .as_vector()
                        .dot(face_normals[uses[right].face].as_vector())?
                        .clamp(-1.0, 1.0);
                    if dot < minimum_dot {
                        continue;
                    }
                    for endpoint in 0..2 {
                        union_indices_keep_later(
                            &mut parents,
                            uses[left].raw_vertices[endpoint] as usize,
                            uses[right].raw_vertices[endpoint] as usize,
                        );
                    }
                }
            }
        }

        let mut retained = vec![false; self.vertices.len()];
        for face in &self.faces {
            for &vertex in face.indices() {
                let representative = index_root(&mut parents, vertex as usize);
                retained[representative] = true;
            }
        }
        let retained_count = retained.iter().filter(|&&keep| keep).count();
        let removed = self.vertices.len() - retained_count;
        if removed == 0 {
            return Ok((self.clone(), 0));
        }

        let mut representative_remap = vec![0_u32; self.vertices.len()];
        let mut vertices = Vec::with_capacity(retained_count);
        for (source, (&point, keep)) in self.vertices.iter().zip(retained).enumerate() {
            if !keep {
                continue;
            }
            representative_remap[source] = u32::try_from(vertices.len())
                .expect("a welded mesh cannot have more vertices than its source");
            vertices.push(point);
        }
        let faces = self
            .faces
            .iter()
            .copied()
            .map(|face| {
                face.remapped(|vertex| {
                    let representative = index_root(&mut parents, vertex as usize);
                    representative_remap[representative]
                })
            })
            .collect();
        Ok((Self::from_validated_parts(vertices, faces), removed))
    }

    /// Separates coincident edge endpoints where the incident-face normal
    /// angle is greater than or equal to the supplied tolerance.
    ///
    /// Face regions that remain connected through smoother, already-welded
    /// edges continue to share a raw vertex. Qualifying topology vertices are
    /// rebuilt in OpenNURBS radial order, and unused vertices are compacted as
    /// Rhino does even when no edge qualifies. The returned count is the
    /// number of qualifying topology edges.
    pub fn unwelded_vertices(
        &self,
        angle_tolerance_radians: Real,
    ) -> Result<(Self, usize), GeometryError> {
        if !(angle_tolerance_radians.is_finite()
            && (0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians))
        {
            return Err(GeometryError::InvalidMeshUnweldAngle);
        }
        let data = self.topology_data();
        let face_normals = self.polygon_face_normals()?;
        let maximum_dot = angle_tolerance_radians.cos();
        let edges = data
            .edges
            .iter()
            .map(|(&(first, second), incidence)| ([first, second], incidence))
            .collect::<Vec<_>>();
        let mut qualifying_edges = vec![false; edges.len()];
        for (edge_index, (_, incidence)) in edges.iter().enumerate() {
            let uses = incidence.uses().collect::<Vec<_>>();
            'pairs: for left in 0..uses.len().saturating_sub(1) {
                for right in left + 1..uses.len() {
                    let dot = face_normals[uses[left].face]
                        .as_vector()
                        .dot(face_normals[uses[right].face].as_vector())?
                        .clamp(-1.0, 1.0);
                    if dot <= maximum_dot {
                        qualifying_edges[edge_index] = true;
                        break 'pairs;
                    }
                }
            }
        }
        let qualifying_edge_count = qualifying_edges.iter().filter(|&&edge| edge).count();
        if qualifying_edge_count == 0 {
            return Ok((self.culled_unused_vertices().0, 0));
        }

        let mut affected_topological_vertices = vec![false; data.topological_vertex_count];
        for (edge, qualifies) in edges.iter().zip(&qualifying_edges) {
            if *qualifies {
                affected_topological_vertices[edge.0[0]] = true;
                affected_topological_vertices[edge.0[1]] = true;
            }
        }
        let face_edges = topology_face_edge_indices(self, &data);
        let mut incident_edges = vec![Vec::new(); data.topological_vertex_count];
        for (edge, (vertices, _)) in edges.iter().enumerate() {
            incident_edges[vertices[0]].push(edge);
            incident_edges[vertices[1]].push(edge);
        }
        let mut edge_groups = vec![Vec::new(); data.topological_vertex_count];
        for vertex in 0..data.topological_vertex_count {
            if affected_topological_vertices[vertex] {
                edge_groups[vertex] = radially_sorted_vertex_edges(
                    vertex,
                    &incident_edges[vertex],
                    &edges,
                    &face_edges,
                );
            }
        }

        let mut incident_faces = vec![Vec::new(); data.topological_vertex_count];
        for (face, polygon) in self.faces.iter().enumerate() {
            for &raw in polygon.indices() {
                incident_faces[data.topological_vertices[raw as usize]].push(face);
            }
        }
        let mut face_components = vec![Vec::new(); data.topological_vertex_count];
        for topological_vertex in 0..data.topological_vertex_count {
            if !affected_topological_vertices[topological_vertex] {
                continue;
            }
            let vertex_faces = &incident_faces[topological_vertex];
            let face_to_local = vertex_faces
                .iter()
                .enumerate()
                .map(|(local, &face)| (face, local))
                .collect::<BTreeMap<_, _>>();
            let mut parents = (0..vertex_faces.len()).collect::<Vec<_>>();
            let mut ranks = vec![0_u8; vertex_faces.len()];
            for &edge in &incident_edges[topological_vertex] {
                let (edge_vertices, incidence) = edges[edge];
                let endpoint = if edge_vertices[0] == topological_vertex {
                    Some(0)
                } else if edge_vertices[1] == topological_vertex {
                    Some(1)
                } else {
                    None
                };
                let Some(endpoint) = endpoint else {
                    continue;
                };
                let uses = incidence.uses().collect::<Vec<_>>();
                for left in 0..uses.len().saturating_sub(1) {
                    for right in left + 1..uses.len() {
                        if uses[left].raw_vertices[endpoint] != uses[right].raw_vertices[endpoint] {
                            continue;
                        }
                        let dot = face_normals[uses[left].face]
                            .as_vector()
                            .dot(face_normals[uses[right].face].as_vector())?
                            .clamp(-1.0, 1.0);
                        if dot > maximum_dot {
                            union_faces(
                                &mut parents,
                                &mut ranks,
                                face_to_local[&uses[left].face],
                                face_to_local[&uses[right].face],
                            );
                        }
                    }
                }
            }

            let component_order = ordered_vertex_face_components(
                &edge_groups[topological_vertex],
                &edges,
                &face_to_local,
                vertex_faces,
                &mut parents,
            );
            for component in component_order {
                let mut faces = Vec::new();
                for &face in vertex_faces {
                    let local = face_to_local[&face];
                    if face_root(&mut parents, local) == component {
                        faces.push(face);
                    }
                }
                face_components[topological_vertex].push(faces);
            }
        }
        let vertex_order = (0..data.topological_vertex_count).collect::<Vec<_>>();
        Ok((
            self.rebuilt_from_face_components(&data, &face_components, &vertex_order)?,
            qualifying_edge_count,
        ))
    }

    /// Separates the supplied exact-location topology edges.
    ///
    /// Indices use the same deterministic order as [`Self::wireframe_lines`].
    /// Naked and already-unwelded edges require no new seams, but a non-empty
    /// valid selection still compacts unused vertices as Rhino does. The
    /// returned count is the number of distinct edges that required a new
    /// separation.
    pub fn unwelded_topology_edges(
        &self,
        edge_indices: &[usize],
    ) -> Result<(Self, usize), GeometryError> {
        if edge_indices.is_empty() {
            return Ok((self.clone(), 0));
        }
        let data = self.topology_data();
        let edges = data
            .edges
            .iter()
            .map(|(&(first, second), incidence)| ([first, second], incidence))
            .collect::<Vec<_>>();
        let mut selected_edges = vec![false; edges.len()];
        for &edge in edge_indices {
            let Some(selected) = selected_edges.get_mut(edge) else {
                return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                    edge,
                    edge_count: edges.len(),
                });
            };
            *selected = true;
        }
        let active_edges = edges
            .iter()
            .zip(selected_edges)
            .map(|((_, incidence), selected)| {
                selected
                    && incidence.count > 1
                    && !edge_uses_are_unwelded(&incidence.uses().collect::<Vec<_>>())
            })
            .collect::<Vec<_>>();
        let active_edge_count = active_edges.iter().filter(|&&active| active).count();
        if active_edge_count == 0 {
            return Ok((self.culled_unused_vertices().0, 0));
        }

        let mut affected_topological_vertices = vec![false; data.topological_vertex_count];
        let mut incident_edges = vec![Vec::new(); data.topological_vertex_count];
        for (edge, (vertices, _)) in edges.iter().enumerate() {
            incident_edges[vertices[0]].push(edge);
            incident_edges[vertices[1]].push(edge);
            if active_edges[edge] {
                affected_topological_vertices[vertices[0]] = true;
                affected_topological_vertices[vertices[1]] = true;
            }
        }
        let face_edges = topology_face_edge_indices(self, &data);
        let mut edge_groups = vec![Vec::new(); data.topological_vertex_count];
        for vertex in 0..data.topological_vertex_count {
            if affected_topological_vertices[vertex] {
                edge_groups[vertex] = radially_sorted_vertex_edges(
                    vertex,
                    &incident_edges[vertex],
                    &edges,
                    &face_edges,
                );
            }
        }
        let mut incident_faces = vec![Vec::new(); data.topological_vertex_count];
        for (face, polygon) in self.faces.iter().enumerate() {
            for &raw in polygon.indices() {
                incident_faces[data.topological_vertices[raw as usize]].push(face);
            }
        }

        let mut face_components = vec![Vec::new(); data.topological_vertex_count];
        for topological_vertex in 0..data.topological_vertex_count {
            if !affected_topological_vertices[topological_vertex] {
                continue;
            }
            let vertex_faces = &incident_faces[topological_vertex];
            let face_to_local = vertex_faces
                .iter()
                .enumerate()
                .map(|(local, &face)| (face, local))
                .collect::<BTreeMap<_, _>>();
            let mut separated_edges = active_edges.clone();
            for group in &edge_groups[topological_vertex] {
                let selected = group
                    .iter()
                    .enumerate()
                    .filter_map(|(position, &edge)| active_edges[edge].then_some(position))
                    .collect::<Vec<_>>();
                let closed_manifold = group.iter().all(|&edge| edges[edge].1.count == 2);
                if closed_manifold && selected.len() == 1 {
                    separated_edges[group[(selected[0] + 1) % group.len()]] = true;
                }
            }

            let mut parents = (0..vertex_faces.len()).collect::<Vec<_>>();
            let mut ranks = vec![0_u8; vertex_faces.len()];
            for &edge in &incident_edges[topological_vertex] {
                if separated_edges[edge] {
                    continue;
                }
                let (edge_vertices, incidence) = edges[edge];
                let endpoint = usize::from(edge_vertices[1] == topological_vertex);
                debug_assert_eq!(edge_vertices[endpoint], topological_vertex);
                let uses = incidence.uses().collect::<Vec<_>>();
                for left in 0..uses.len().saturating_sub(1) {
                    for right in left + 1..uses.len() {
                        if uses[left].raw_vertices[endpoint] == uses[right].raw_vertices[endpoint] {
                            union_faces(
                                &mut parents,
                                &mut ranks,
                                face_to_local[&uses[left].face],
                                face_to_local[&uses[right].face],
                            );
                        }
                    }
                }
            }

            let mut component_order = ordered_vertex_face_components(
                &edge_groups[topological_vertex],
                &edges,
                &face_to_local,
                vertex_faces,
                &mut parents,
            );
            component_order.reverse();
            for component in component_order {
                let mut faces = Vec::new();
                for &face in vertex_faces {
                    if face_root(&mut parents, face_to_local[&face]) == component {
                        faces.push(face);
                    }
                }
                face_components[topological_vertex].push(faces);
            }
        }

        let vertex_order =
            topology_edge_vertex_order(data.topological_vertex_count, &edges, &active_edges);
        Ok((
            self.rebuilt_from_face_components(&data, &face_components, &vertex_order)?,
            active_edge_count,
        ))
    }

    fn rebuilt_from_face_components(
        &self,
        data: &MeshTopologyData,
        face_components: &[Vec<Vec<usize>>],
        topological_vertex_order: &[usize],
    ) -> Result<Self, GeometryError> {
        let affected_topological_vertices = face_components
            .iter()
            .map(|components| !components.is_empty())
            .collect::<Vec<_>>();
        let mut used = vec![false; self.vertices.len()];
        for face in &self.faces {
            for &vertex in face.indices() {
                used[vertex as usize] = true;
            }
        }
        let mut raw_remap = vec![u32::MAX; self.vertices.len()];
        let mut vertices = Vec::new();
        for (source, (&point, is_used)) in self.vertices.iter().zip(used).enumerate() {
            if !is_used || affected_topological_vertices[data.topological_vertices[source]] {
                continue;
            }
            raw_remap[source] =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            vertices.push(point);
        }

        let mut face_replacements = vec![BTreeMap::<u32, u32>::new(); self.faces.len()];
        for (face_index, face) in self.faces.iter().enumerate() {
            for &raw_vertex in face.indices() {
                let source = raw_vertex as usize;
                if !affected_topological_vertices[data.topological_vertices[source]] {
                    face_replacements[face_index].insert(raw_vertex, raw_remap[source]);
                }
            }
        }
        for &topological_vertex in topological_vertex_order {
            let components = &face_components[topological_vertex];
            for component in components {
                let target = u32::try_from(vertices.len())
                    .map_err(|_| GeometryError::TooManyMeshVertices)?;
                vertices.push(data.topological_points[topological_vertex]);
                for &face in component {
                    let raw_vertex = self.faces[face]
                        .indices()
                        .iter()
                        .copied()
                        .find(|&raw| data.topological_vertices[raw as usize] == topological_vertex)
                        .expect("an incident face contains its topology vertex");
                    face_replacements[face].insert(raw_vertex, target);
                }
            }
        }

        let faces = self
            .faces
            .iter()
            .copied()
            .enumerate()
            .map(|(face, polygon)| {
                polygon.remapped(|raw| {
                    *face_replacements[face]
                        .get(&raw)
                        .expect("every unwelded face vertex has a replacement")
                })
            })
            .collect();
        Ok(Self::from_validated_parts(vertices, faces))
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
            for side in 0..indices.len() {
                let raw_from = indices[side];
                let raw_to = indices[(side + 1) % indices.len()];
                let from = topological_vertices[raw_from as usize];
                let to = topological_vertices[raw_to as usize];
                debug_assert_ne!(from, to, "validated mesh edge collapsed");
                let (edge, forward, raw_vertices) = if from < to {
                    ((from, to), true, [raw_from, raw_to])
                } else {
                    ((to, from), false, [raw_to, raw_from])
                };
                let incidence = edges.entry(edge).or_default();
                incidence.add_use(EdgeUse {
                    face: face_index,
                    side,
                    forward,
                    raw_vertices,
                });
            }
        }

        MeshTopologyData {
            topological_vertex_count: locations.len(),
            topological_points,
            topological_vertices,
            edges,
        }
    }

    fn polygon_face_normals(&self) -> Result<Vec<UnitVector3>, GeometryError> {
        self.faces
            .iter()
            .map(|face| match *face {
                MeshFace::Triangle([a, b, c]) => self.vertices[a as usize]
                    .vector_to(self.vertices[b as usize])?
                    .cross(self.vertices[a as usize].vector_to(self.vertices[c as usize])?)?
                    .normalized_nonzero(),
                MeshFace::Quad([a, b, c, d]) => self.vertices[a as usize]
                    .vector_to(self.vertices[c as usize])?
                    .cross(self.vertices[b as usize].vector_to(self.vertices[d as usize])?)?
                    .normalized_nonzero(),
            })
            .collect()
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

fn validate_mesh_edge_filter(filter: MeshEdgeFilter) -> Result<(), GeometryError> {
    let MeshEdgeFilter::FaceAngle {
        greater_than_radians,
        less_than_radians,
    } = filter
    else {
        return Ok(());
    };
    if greater_than_radians.is_finite()
        && less_than_radians.is_finite()
        && greater_than_radians >= 0.0
        && greater_than_radians < less_than_radians
        && less_than_radians <= std::f64::consts::PI
    {
        Ok(())
    } else {
        Err(GeometryError::InvalidMeshFaceAngleInterval)
    }
}

fn validate_mesh_break_angle(break_angle_radians: Real) -> Result<(), GeometryError> {
    if break_angle_radians.is_finite()
        && (0.0..=std::f64::consts::PI).contains(&break_angle_radians)
    {
        Ok(())
    } else {
        Err(GeometryError::InvalidMeshBreakAngle)
    }
}

fn mesh_edge_is_logical_boundary(
    incidence: &EdgeIncidence,
    face_normals: &[UnitVector3],
    break_angle_radians: Real,
) -> bool {
    let uses = incidence.uses().collect::<Vec<_>>();
    if uses.len() == 1 || edge_uses_are_unwelded(&uses) {
        return true;
    }
    (0..uses.len() - 1).any(|left| {
        (left + 1..uses.len()).any(|right| {
            unit_vector_angle(
                face_normals[uses[left].face],
                face_normals[uses[right].face],
            ) >= break_angle_radians
        })
    })
}

fn unit_vector_angle(first: UnitVector3, second: UnitVector3) -> Real {
    first
        .as_vector()
        .dot(second.as_vector())
        .expect("finite unit-vector dot products cannot fail")
        .clamp(-1.0, 1.0)
        .acos()
}

fn mesh_edge_matches_filter(
    incidence: &EdgeIncidence,
    filter: MeshEdgeFilter,
    face_normals: Option<&[UnitVector3]>,
) -> bool {
    match filter {
        MeshEdgeFilter::Naked => incidence.count == 1,
        MeshEdgeFilter::Unwelded => {
            incidence.count == 1 || edge_uses_are_unwelded(&incidence.uses().collect::<Vec<_>>())
        }
        MeshEdgeFilter::FaceAngle {
            greater_than_radians,
            less_than_radians,
        } => {
            let uses = incidence.uses().collect::<Vec<_>>();
            if uses.len() < 2 {
                return false;
            }
            let normals = face_normals.expect("face-angle filtering computes polygon normals");
            let mut greatest_angle: Real = 0.0;
            for left in 0..uses.len() - 1 {
                for right in left + 1..uses.len() {
                    greatest_angle = greatest_angle.max(unit_vector_angle(
                        normals[uses[left].face],
                        normals[uses[right].face],
                    ));
                }
            }
            greatest_angle > greater_than_radians && greatest_angle < less_than_radians
        }
    }
}

#[derive(Clone, Copy)]
struct AugmentedTopologyEdge {
    vertices: [usize; 2],
    virtual_edge: bool,
}

fn topology_edge_polylines(
    points: &[Point3],
    edges: &[[usize; 2]],
    tolerance: Tolerance,
) -> Result<Vec<Polyline3>, GeometryError> {
    let components = topology_edge_components(points.len(), edges);
    let mut polylines = Vec::new();
    for component in components {
        let mut degrees = vec![0_usize; points.len()];
        let mut augmented = component
            .iter()
            .map(|&edge| {
                let vertices = edges[edge];
                degrees[vertices[0]] += 1;
                degrees[vertices[1]] += 1;
                AugmentedTopologyEdge {
                    vertices,
                    virtual_edge: false,
                }
            })
            .collect::<Vec<_>>();
        let odd_vertices = degrees
            .iter()
            .enumerate()
            .filter_map(|(vertex, &degree)| (degree % 2 == 1).then_some(vertex))
            .collect::<Vec<_>>();
        debug_assert_eq!(odd_vertices.len() % 2, 0);
        for pair in odd_vertices.chunks_exact(2) {
            augmented.push(AugmentedTopologyEdge {
                vertices: [pair[0], pair[1]],
                virtual_edge: true,
            });
        }

        let start = odd_vertices
            .first()
            .copied()
            .unwrap_or(augmented[0].vertices[0]);
        let (walk_vertices, walk_edges) = topology_euler_circuit(points.len(), &augmented, start);
        let virtual_positions = walk_edges
            .iter()
            .enumerate()
            .filter_map(|(position, &edge)| augmented[edge].virtual_edge.then_some(position))
            .collect::<Vec<_>>();
        if virtual_positions.is_empty() {
            polylines.push(Polyline3::try_new(
                walk_vertices
                    .into_iter()
                    .map(|vertex| points[vertex])
                    .collect(),
                tolerance,
            )?);
            continue;
        }

        let edge_count = walk_edges.len();
        for (position, &virtual_position) in virtual_positions.iter().enumerate() {
            let stop = virtual_positions[(position + 1) % virtual_positions.len()];
            let mut edge_position = (virtual_position + 1) % edge_count;
            let mut trail = vec![points[walk_vertices[edge_position]]];
            while edge_position != stop {
                debug_assert!(!augmented[walk_edges[edge_position]].virtual_edge);
                trail.push(points[walk_vertices[(edge_position + 1) % edge_count]]);
                edge_position = (edge_position + 1) % edge_count;
            }
            if trail.len() >= 2 {
                polylines.push(Polyline3::try_new(trail, tolerance)?);
            }
        }
    }
    Ok(polylines)
}

fn topology_edge_components(vertex_count: usize, edges: &[[usize; 2]]) -> Vec<Vec<usize>> {
    let mut adjacency = vec![Vec::new(); vertex_count];
    for (edge, vertices) in edges.iter().copied().enumerate() {
        adjacency[vertices[0]].push(edge);
        adjacency[vertices[1]].push(edge);
    }
    let mut visited = vec![false; edges.len()];
    let mut components = Vec::new();
    for first in 0..edges.len() {
        if visited[first] {
            continue;
        }
        visited[first] = true;
        let mut pending = vec![first];
        let mut component = Vec::new();
        while let Some(edge) = pending.pop() {
            component.push(edge);
            for vertex in edges[edge] {
                for &neighbor in &adjacency[vertex] {
                    if !visited[neighbor] {
                        visited[neighbor] = true;
                        pending.push(neighbor);
                    }
                }
            }
        }
        component.sort_unstable();
        components.push(component);
    }
    components
}

fn topology_edge_vertex_order(
    vertex_count: usize,
    edges: &[([usize; 2], &EdgeIncidence)],
    selected: &[bool],
) -> Vec<usize> {
    let selected_edges = edges
        .iter()
        .zip(selected)
        .filter_map(|((vertices, _), &selected)| selected.then_some(*vertices))
        .collect::<Vec<_>>();
    let mut order = Vec::new();
    let mut seen = vec![false; vertex_count];
    for component in topology_edge_components(vertex_count, &selected_edges) {
        let mut degrees = vec![0_usize; vertex_count];
        let mut augmented = component
            .iter()
            .map(|&edge| {
                let vertices = selected_edges[edge];
                degrees[vertices[0]] += 1;
                degrees[vertices[1]] += 1;
                AugmentedTopologyEdge {
                    vertices,
                    virtual_edge: false,
                }
            })
            .collect::<Vec<_>>();
        let odd_vertices = degrees
            .iter()
            .enumerate()
            .filter_map(|(vertex, &degree)| (degree % 2 == 1).then_some(vertex))
            .collect::<Vec<_>>();
        for pair in odd_vertices.chunks_exact(2) {
            augmented.push(AugmentedTopologyEdge {
                vertices: [pair[0], pair[1]],
                virtual_edge: true,
            });
        }
        let start = odd_vertices
            .first()
            .copied()
            .unwrap_or(augmented[0].vertices[0]);
        let (walk, _) = topology_euler_circuit(vertex_count, &augmented, start);
        for vertex in walk {
            if !seen[vertex] {
                seen[vertex] = true;
                order.push(vertex);
            }
        }
    }
    order.extend(
        seen.into_iter()
            .enumerate()
            .filter_map(|(vertex, seen)| (!seen).then_some(vertex)),
    );
    order
}

fn topology_euler_circuit(
    vertex_count: usize,
    edges: &[AugmentedTopologyEdge],
    start: usize,
) -> (Vec<usize>, Vec<usize>) {
    let mut adjacency = vec![Vec::new(); vertex_count];
    for (edge, topology_edge) in edges.iter().enumerate() {
        adjacency[topology_edge.vertices[0]].push(edge);
        adjacency[topology_edge.vertices[1]].push(edge);
    }
    let mut next_adjacency = vec![0_usize; vertex_count];
    let mut used = vec![false; edges.len()];
    let mut vertex_stack = vec![start];
    let mut edge_stack = Vec::new();
    let mut reversed_vertices = Vec::with_capacity(edges.len() + 1);
    let mut reversed_edges = Vec::with_capacity(edges.len());
    while let Some(&vertex) = vertex_stack.last() {
        while next_adjacency[vertex] < adjacency[vertex].len()
            && used[adjacency[vertex][next_adjacency[vertex]]]
        {
            next_adjacency[vertex] += 1;
        }
        if let Some(&edge) = adjacency[vertex].get(next_adjacency[vertex]) {
            used[edge] = true;
            next_adjacency[vertex] += 1;
            let [first, second] = edges[edge].vertices;
            vertex_stack.push(if vertex == first { second } else { first });
            edge_stack.push(edge);
        } else {
            reversed_vertices.push(
                vertex_stack
                    .pop()
                    .expect("an Euler traversal has a current vertex"),
            );
            if let Some(edge) = edge_stack.pop() {
                reversed_edges.push(edge);
            }
        }
    }
    debug_assert!(used.into_iter().all(|edge| edge));
    reversed_vertices.reverse();
    reversed_edges.reverse();
    (reversed_vertices, reversed_edges)
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

fn edge_uses_are_unwelded(uses: &[EdgeUse]) -> bool {
    let mut first_endpoint_indices = BTreeSet::new();
    let mut second_endpoint_indices = BTreeSet::new();
    for edge_use in uses {
        if !first_endpoint_indices.insert(edge_use.raw_vertices[0])
            || !second_endpoint_indices.insert(edge_use.raw_vertices[1])
        {
            return false;
        }
    }
    true
}

fn topology_face_edge_indices(mesh: &TriangleMesh, data: &MeshTopologyData) -> Vec<Vec<usize>> {
    let edge_indices = data
        .edges
        .keys()
        .copied()
        .enumerate()
        .map(|(index, edge)| (edge, index))
        .collect::<BTreeMap<_, _>>();
    mesh.faces
        .iter()
        .map(|face| {
            let raw_vertices = face.indices();
            (0..raw_vertices.len())
                .map(|side| {
                    let first = data.topological_vertices[raw_vertices[side] as usize];
                    let second = data.topological_vertices
                        [raw_vertices[(side + 1) % raw_vertices.len()] as usize];
                    edge_indices[&(first.min(second), first.max(second))]
                })
                .collect()
        })
        .collect()
}

/// Reproduces `ON_MeshTopology::SortVertexEdges`: each returned group is one
/// radial fan, starting at a naked/non-manifold edge when one is present.
fn radially_sorted_vertex_edges(
    topological_vertex: usize,
    incident_edges: &[usize],
    edges: &[([usize; 2], &EdgeIncidence)],
    face_edges: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    let mut naked = Vec::new();
    let mut manifold = Vec::new();
    let mut non_manifold = Vec::new();
    for &edge in incident_edges {
        let (vertices, incidence) = edges[edge];
        debug_assert!(vertices.contains(&topological_vertex));
        match incidence.count {
            1 => naked.push(edge),
            2 => manifold.push(edge),
            _ => non_manifold.push(edge),
        }
    }
    naked.extend(non_manifold);

    let mut groups = Vec::new();
    while !naked.is_empty() || !manifold.is_empty() {
        let first = if naked.is_empty() {
            manifold.remove(0)
        } else {
            naked.remove(0)
        };
        let mut group = vec![first];
        let mut current = first;
        let mut group_direction = 0_i8;
        loop {
            let (vertices, incidence) = edges[current];
            let mut next = None;
            for edge_use in incidence.uses() {
                let direction = if vertices[0] == topological_vertex {
                    if edge_use.forward { -1 } else { 1 }
                } else if edge_use.forward {
                    1
                } else {
                    -1
                };
                let side_count = face_edges[edge_use.face].len();
                let next_side = if direction < 0 {
                    (edge_use.side + side_count - 1) % side_count
                } else {
                    (edge_use.side + 1) % side_count
                };
                let candidate = face_edges[edge_use.face][next_side];
                let removed = if let Some(index) = naked.iter().position(|&edge| edge == candidate)
                {
                    naked.remove(index);
                    true
                } else if let Some(index) = manifold.iter().position(|&edge| edge == candidate) {
                    manifold.remove(index);
                    true
                } else {
                    false
                };
                if removed {
                    if group_direction == 0 {
                        group_direction = direction;
                    }
                    next = Some(candidate);
                    break;
                }
            }
            let Some(next_edge) = next else {
                break;
            };
            group.push(next_edge);
            current = next_edge;
        }
        if group_direction > 0 {
            group.reverse();
        }
        groups.push(group);
    }
    groups
}

fn shared_edge_face(first: &EdgeIncidence, second: &EdgeIncidence) -> Option<usize> {
    first.uses().find_map(|first_use| {
        second
            .uses()
            .any(|second_use| second_use.face == first_use.face)
            .then_some(first_use.face)
    })
}

fn ordered_vertex_face_components(
    edge_groups: &[Vec<usize>],
    edges: &[([usize; 2], &EdgeIncidence)],
    face_to_local: &BTreeMap<usize, usize>,
    incident_faces: &[usize],
    parents: &mut [usize],
) -> Vec<usize> {
    let mut order = Vec::new();
    let mut seen = BTreeSet::new();
    for edge_group in edge_groups {
        let mut radial_roots = Vec::new();
        let mut add_shared_root = |first: usize, second: usize| {
            let Some(face) = shared_edge_face(edges[first].1, edges[second].1) else {
                return;
            };
            let Some(&local) = face_to_local.get(&face) else {
                return;
            };
            let root = face_root(parents, local);
            if radial_roots.last().copied() != Some(root) {
                radial_roots.push(root);
            }
        };
        for pair in edge_group.windows(2) {
            add_shared_root(pair[0], pair[1]);
        }
        if edge_group.len() > 1 {
            add_shared_root(*edge_group.last().unwrap(), edge_group[0]);
        }
        if radial_roots.len() > 1 && radial_roots.first() == radial_roots.last() {
            radial_roots.remove(0);
        }
        for root in radial_roots {
            if seen.insert(root) {
                order.push(root);
            }
        }
    }
    for &face in incident_faces {
        let root = face_root(parents, face_to_local[&face]);
        if seen.insert(root) {
            order.push(root);
        }
    }
    order
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

fn index_root(parents: &mut [usize], index: usize) -> usize {
    let mut root = index;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = index;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn union_indices_keep_later(parents: &mut [usize], first: usize, second: usize) {
    let first = index_root(parents, first);
    let second = index_root(parents, second);
    if first < second {
        parents[first] = second;
    } else if second < first {
        parents[second] = first;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn topology_edge_is_unwelded_between(
        mesh: &TriangleMesh,
        first: Point3,
        second: Point3,
    ) -> bool {
        let data = mesh.topology_data();
        let (_, incidence) = data
            .edges
            .iter()
            .find(|&(&(a, b), _)| {
                (data.topological_points[a] == first && data.topological_points[b] == second)
                    || (data.topological_points[a] == second && data.topological_points[b] == first)
            })
            .expect("test topology edge exists");
        incidence.count > 1 && edge_uses_are_unwelded(&incidence.uses().collect::<Vec<_>>())
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
    fn filters_naked_unwelded_and_face_angle_edges() {
        let welded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            welded
                .filtered_edge_lines(MeshEdgeFilter::Naked, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            welded
                .filtered_edge_lines(MeshEdgeFilter::Unwelded, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            4
        );

        let unwelded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            unwelded
                .filtered_edge_lines(MeshEdgeFilter::Naked, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            unwelded
                .filtered_edge_lines(MeshEdgeFilter::Unwelded, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            5
        );

        let folded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 3.0),
            ],
            vec![[0, 1, 2], [0, 3, 1]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let around_right_angle = MeshEdgeFilter::FaceAngle {
            greater_than_radians: 89.0_f64.to_radians(),
            less_than_radians: 91.0_f64.to_radians(),
        };
        assert_eq!(
            folded
                .filtered_edge_lines(around_right_angle, Tolerance::DEFAULT)
                .unwrap(),
            vec![
                LineSegment::try_new(
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    Tolerance::DEFAULT,
                )
                .unwrap()
            ]
        );
        for strict_boundary in [
            MeshEdgeFilter::FaceAngle {
                greater_than_radians: 90.0_f64.to_radians(),
                less_than_radians: 91.0_f64.to_radians(),
            },
            MeshEdgeFilter::FaceAngle {
                greater_than_radians: 89.0_f64.to_radians(),
                less_than_radians: 90.0_f64.to_radians(),
            },
        ] {
            assert!(
                folded
                    .filtered_edge_lines(strict_boundary, Tolerance::DEFAULT)
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn joins_branched_unwelded_edges_into_edge_exact_euler_trails() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let polylines = mesh
            .filtered_edge_polylines(MeshEdgeFilter::Unwelded, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(polylines.len(), 1);
        assert_eq!(
            polylines[0].vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 3.0, 0.0),
            ]
        );
        assert_eq!(polylines[0].segment_count(), 5);
    }

    #[test]
    fn decomposes_many_odd_edge_vertices_without_losing_edges() {
        let points = [
            point(0.0, 0.0, 0.0),
            point(-1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, -1.0, 0.0),
        ];
        let edges = [[0, 1], [0, 2], [0, 3], [0, 4]];
        let polylines = topology_edge_polylines(&points, &edges, Tolerance::DEFAULT).unwrap();
        assert_eq!(polylines.len(), 2);
        assert_eq!(
            polylines
                .iter()
                .map(Polyline3::segment_count)
                .sum::<usize>(),
            edges.len()
        );
        let mut actual = BTreeSet::new();
        for segment in polylines.iter().flat_map(Polyline3::segments) {
            let mut vertices = [segment.start(), segment.end()].map(|point| {
                points
                    .iter()
                    .position(|candidate| *candidate == point)
                    .unwrap()
            });
            vertices.sort_unstable();
            assert!(actual.insert(vertices));
        }
        assert_eq!(actual, edges.into_iter().collect());
    }

    #[test]
    fn logical_edges_join_only_across_locally_smooth_face_regions() {
        let planar = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let split = planar
            .logical_edge_polylines(0.0, Tolerance::DEFAULT)
            .unwrap();
        let mut split_segment_counts = split
            .iter()
            .map(Polyline3::segment_count)
            .collect::<Vec<_>>();
        split_segment_counts.sort_unstable();
        assert_eq!(split_segment_counts, [1, 2, 2]);
        assert_eq!(split_segment_counts.iter().sum::<usize>(), 5);

        let joined = planar
            .logical_edge_polylines(1.0_f64.to_radians(), Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(joined.len(), 1);
        assert!(joined[0].is_closed());
        assert_eq!(joined[0].segment_count(), 4);
    }

    #[test]
    fn logical_edges_use_a_strict_face_grouping_break_angle() {
        let folded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 3.0),
            ],
            vec![[0, 1, 2], [0, 3, 1]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let at_right_angle = folded
            .logical_edge_polylines(90.0_f64.to_radians(), Tolerance::DEFAULT)
            .unwrap();
        let mut segment_counts = at_right_angle
            .iter()
            .map(Polyline3::segment_count)
            .collect::<Vec<_>>();
        segment_counts.sort_unstable();
        assert_eq!(segment_counts, [1, 2, 2]);
        assert_eq!(segment_counts.iter().sum::<usize>(), 5);

        let above_right_angle = folded
            .logical_edge_polylines(91.0_f64.to_radians(), Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(above_right_angle.len(), 1);
        assert!(above_right_angle[0].is_closed());
        assert_eq!(above_right_angle[0].segment_count(), 4);
    }

    #[test]
    fn logical_edges_never_smooth_across_an_unwelded_seam() {
        let unwelded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edges = unwelded
            .logical_edge_polylines(std::f64::consts::PI, Tolerance::DEFAULT)
            .unwrap();
        let mut segment_counts = edges
            .iter()
            .map(Polyline3::segment_count)
            .collect::<Vec<_>>();
        segment_counts.sort_unstable();
        assert_eq!(segment_counts, [1, 2, 2]);
        assert_eq!(segment_counts.iter().sum::<usize>(), 5);
    }

    #[test]
    fn rejects_invalid_mesh_break_angles() {
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
        for break_angle in [-1.0, std::f64::consts::PI.next_up(), Real::NAN] {
            assert_eq!(
                mesh.logical_edge_polylines(break_angle, Tolerance::DEFAULT),
                Err(GeometryError::InvalidMeshBreakAngle)
            );
        }
        assert!(mesh.logical_edge_polylines(0.0, Tolerance::DEFAULT).is_ok());
        assert!(
            mesh.logical_edge_polylines(std::f64::consts::PI, Tolerance::DEFAULT)
                .is_ok()
        );
    }

    #[test]
    fn rejects_invalid_mesh_face_angle_intervals() {
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
        for (greater, less) in [
            (-1.0, 1.0),
            (1.0, 1.0),
            (0.0, std::f64::consts::PI + 1.0),
            (Real::NAN, 1.0),
            (0.0, Real::INFINITY),
        ] {
            assert_eq!(
                mesh.filtered_edge_lines(
                    MeshEdgeFilter::FaceAngle {
                        greater_than_radians: greater,
                        less_than_radians: less,
                    },
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidMeshFaceAngleInterval)
            );
        }
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
    fn explode_pieces_split_only_edges_unwelded_at_both_raw_endpoints() {
        let welded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 2.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(welded.explode_pieces(), vec![welded.clone()]);

        let unwelded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 2.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(unwelded.disjoint_pieces().len(), 1);
        let pieces = unwelded.explode_pieces();
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].triangles(), &[[0, 1, 2]]);
        assert_eq!(pieces[1].triangles(), &[[0, 1, 2]]);
        assert_eq!(pieces[0].vertices(), &unwelded.vertices()[..3]);
        assert_eq!(pieces[1].vertices(), &unwelded.vertices()[3..]);

        let half_welded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 0.0, 2.0),
            ],
            vec![[0, 1, 2], [3, 0, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(half_welded.explode_pieces(), vec![half_welded.clone()]);
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
    fn welds_smooth_coincident_edge_endpoints_in_rhino_order() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (welded, removed) = mesh.welded_vertices(0.0).unwrap();
        assert_eq!(removed, 3);
        assert_eq!(
            welded.vertices(),
            &[
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
            ]
        );
        assert_eq!(welded.triangles(), &[[2, 1, 0], [1, 2, 3]]);
        assert_eq!(welded.topology().edge_count(), 5);
        assert_eq!(welded.explode_pieces().len(), 1);
    }

    #[test]
    fn weld_respects_angle_and_never_merges_a_vertex_only_contact() {
        let right_angle = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 3.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (below, removed) = right_angle.welded_vertices(89.0_f64.to_radians()).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(below.vertices(), &right_angle.vertices()[..6]);
        assert_eq!(below.triangles(), right_angle.triangles());
        let (above, removed) = right_angle
            .welded_vertices(90.001_f64.to_radians())
            .unwrap();
        assert_eq!(removed, 3);
        assert_eq!(above.triangles(), &[[2, 1, 0], [1, 2, 3]]);

        let vertex_only = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(-2.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            vertex_only.welded_vertices(std::f64::consts::PI),
            Ok((vertex_only.clone(), 0))
        );
    }

    #[test]
    fn weld_rejects_invalid_angle_tolerances() {
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
        for angle in [-1.0, std::f64::consts::PI.next_up(), Real::NAN] {
            assert_eq!(
                mesh.welded_vertices(angle),
                Err(GeometryError::InvalidMeshWeldAngle)
            );
        }
    }

    #[test]
    fn unwelds_threshold_edges_in_rhino_radial_order_and_compacts() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (unwelded, edge_count) = mesh.unwelded_vertices(0.0).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(
            unwelded.vertices(),
            &[
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
            ]
        );
        assert_eq!(unwelded.triangles(), &[[3, 4, 0], [5, 2, 1]]);
        assert_eq!(unwelded.explode_pieces().len(), 2);

        let (smooth, edge_count) = mesh.unwelded_vertices(1.0e-6).unwrap();
        assert_eq!(edge_count, 0);
        assert_eq!(smooth.vertices(), &mesh.vertices()[..4]);
        assert_eq!(smooth.triangles(), mesh.triangles());

        let (rebuilt, edge_count) = unwelded.unwelded_vertices(0.0).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(rebuilt, unwelded);

        let already_unwelded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (rebuilt, edge_count) = already_unwelded.unwelded_vertices(0.0).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(rebuilt, unwelded);
    }

    #[test]
    fn unweld_uses_an_inclusive_angle_threshold() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 3.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (equal, edge_count) = mesh.unwelded_vertices(std::f64::consts::FRAC_PI_2).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(equal.triangles(), &[[3, 4, 0], [5, 2, 1]]);

        let (above, edge_count) = mesh.unwelded_vertices(90.001_f64.to_radians()).unwrap();
        assert_eq!(edge_count, 0);
        assert_eq!(above, mesh);
    }

    #[test]
    fn unweld_partitions_a_closed_corner_into_smooth_face_regions() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
            point(1.0, 0.0, 1.0),
            point(1.0, 1.0, 1.0),
            point(0.0, 1.0, 1.0),
        ];
        let mesh = TriangleMesh::try_new(
            vertices.clone(),
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
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
        let (unwelded, edge_count) = mesh.unwelded_vertices(std::f64::consts::FRAC_PI_4).unwrap();
        assert_eq!(edge_count, 12);
        assert_eq!(unwelded.vertices().len(), 24);
        assert_eq!(
            unwelded.triangles(),
            &[
                [1, 8, 3],
                [1, 10, 8],
                [13, 16, 19],
                [13, 19, 22],
                [2, 5, 17],
                [2, 17, 12],
                [4, 7, 20],
                [4, 20, 15],
                [6, 9, 23],
                [6, 23, 18],
                [11, 0, 14],
                [11, 14, 21],
            ]
        );
        assert!(
            unwelded
                .vertices()
                .chunks_exact(3)
                .all(|copies| { copies[0] == copies[1] && copies[1] == copies[2] })
        );
        assert_eq!(unwelded.explode_pieces().len(), 6);

        let quad_mesh = TriangleMesh::try_new_faces(
            vertices,
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
        let (quad_unwelded, edge_count) = quad_mesh
            .unwelded_vertices(std::f64::consts::FRAC_PI_4)
            .unwrap();
        assert_eq!(edge_count, 12);
        assert_eq!(quad_unwelded.vertices().len(), 24);
        assert!(quad_unwelded.faces().iter().all(|face| face.is_quad()));
        assert_eq!(quad_unwelded.explode_pieces().len(), 6);
    }

    #[test]
    fn unwelds_selected_topology_edges_in_rhino_order_and_compacts() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (unwelded, edge_count) = mesh.unwelded_topology_edges(&[0]).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(
            unwelded.vertices(),
            &[
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
            ]
        );
        assert_eq!(unwelded.triangles(), &[[2, 5, 0], [4, 3, 1]]);
        assert_eq!(mesh.unwelded_topology_edges(&[0, 0]).unwrap().0, unwelded);

        let (empty, edge_count) = mesh.unwelded_topology_edges(&[]).unwrap();
        assert_eq!(edge_count, 0);
        assert_eq!(empty, mesh);
        let (naked, edge_count) = mesh.unwelded_topology_edges(&[1]).unwrap();
        assert_eq!(edge_count, 0);
        assert_eq!(naked.vertices(), &mesh.vertices()[..4]);
        assert_eq!(naked.triangles(), mesh.triangles());

        assert_eq!(
            mesh.unwelded_topology_edges(&[5]),
            Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: 5,
                edge_count: 5,
            })
        );
        let already_unwelded = TriangleMesh::try_new(
            unwelded.vertices().to_vec(),
            unwelded.triangles().to_vec(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            already_unwelded.unwelded_topology_edges(&[0]).unwrap(),
            (already_unwelded.clone(), 0)
        );
    }

    #[test]
    fn selected_edge_splits_closed_radial_fans_deterministically() {
        let fan = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(-1.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
            ],
            vec![[0, 2, 1], [0, 3, 2], [0, 4, 3], [0, 1, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (one, edge_count) = fan.unwelded_topology_edges(&[0]).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(
            one.triangles(),
            &[[4, 0, 5], [4, 1, 0], [4, 2, 1], [3, 6, 2]]
        );
        assert_eq!(one.vertices().len(), 7);

        let (opposite, edge_count) = fan.unwelded_topology_edges(&[0, 2]).unwrap();
        assert_eq!(edge_count, 2);
        assert_eq!(opposite.vertices().len(), 8);
        assert!(topology_edge_is_unwelded_between(
            &opposite,
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0)
        ));
        assert!(topology_edge_is_unwelded_between(
            &opposite,
            point(0.0, 0.0, 0.0),
            point(-1.0, 0.0, 0.0)
        ));
        assert_eq!(opposite.explode_pieces().len(), 2);

        let (adjacent, edge_count) = fan.unwelded_topology_edges(&[0, 1]).unwrap();
        assert_eq!(edge_count, 2);
        assert_eq!(adjacent.vertices().len(), 8);
        assert!(topology_edge_is_unwelded_between(
            &adjacent,
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0)
        ));
        assert!(topology_edge_is_unwelded_between(
            &adjacent,
            point(0.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0)
        ));
        assert_eq!(adjacent.explode_pieces().len(), 2);
    }

    #[test]
    fn selected_edge_loop_detaches_a_cube_face_without_splitting_diagonals() {
        let mesh = TriangleMesh::try_new(
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
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
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
        let (unwelded, edge_count) = mesh.unwelded_topology_edges(&[0, 2, 5, 8]).unwrap();
        assert_eq!(edge_count, 4);
        assert_eq!(unwelded.vertices().len(), 12);
        for (first, second) in [
            (point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0)),
            (point(0.0, 0.0, 0.0), point(0.0, 1.0, 0.0)),
            (point(1.0, 0.0, 0.0), point(1.0, 1.0, 0.0)),
            (point(1.0, 1.0, 0.0), point(0.0, 1.0, 0.0)),
        ] {
            assert!(topology_edge_is_unwelded_between(&unwelded, first, second));
        }
        for (first, second) in [
            (point(0.0, 0.0, 0.0), point(1.0, 1.0, 0.0)),
            (point(0.0, 0.0, 0.0), point(1.0, 0.0, 1.0)),
            (point(1.0, 0.0, 0.0), point(1.0, 1.0, 1.0)),
            (point(1.0, 1.0, 0.0), point(0.0, 1.0, 1.0)),
            (point(0.0, 1.0, 0.0), point(0.0, 0.0, 1.0)),
            (point(0.0, 0.0, 1.0), point(1.0, 1.0, 1.0)),
        ] {
            assert!(!topology_edge_is_unwelded_between(&unwelded, first, second));
        }
        assert_eq!(unwelded.explode_pieces().len(), 2);
    }

    #[test]
    fn unweld_rejects_invalid_angle_tolerances() {
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
        for angle in [-1.0, std::f64::consts::PI.next_up(), Real::NAN] {
            assert_eq!(
                mesh.unwelded_vertices(angle),
                Err(GeometryError::InvalidMeshUnweldAngle)
            );
        }
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
