use std::collections::{BTreeMap, VecDeque};

use crate::{
    AffineTransform3, BoundingBox3, GeometryError, Point3, Real, Tolerance, UnitVector3,
    require_finite,
};

/// Exact location-welded edge topology for a triangle mesh.
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

#[derive(Clone, Copy, Debug, Default)]
struct EdgeIncidence {
    count: usize,
    forward_count: usize,
    first_use: Option<EdgeUse>,
    second_use: Option<EdgeUse>,
}

impl EdgeIncidence {
    fn add_use(&mut self, edge_use: EdgeUse) {
        match self.count {
            0 => self.first_use = Some(edge_use),
            1 => self.second_use = Some(edge_use),
            _ => {}
        }
        self.count += 1;
        self.forward_count += usize::from(edge_use.forward);
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
    edges: BTreeMap<(usize, usize), EdgeIncidence>,
}

/// An indexed, oriented triangle mesh with validated finite vertices and
/// non-degenerate faces.
#[derive(Clone, Debug, PartialEq)]
pub struct TriangleMesh {
    vertices: Vec<Point3>,
    triangles: Vec<[u32; 3]>,
}

impl TriangleMesh {
    pub fn try_new(
        vertices: Vec<Point3>,
        triangles: Vec<[u32; 3]>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if triangles.is_empty() {
            return Err(GeometryError::EmptyMesh);
        }
        if vertices
            .len()
            .checked_sub(1)
            .is_some_and(|last_index| u32::try_from(last_index).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }

        for (triangle_index, triangle) in triangles.iter().copied().enumerate() {
            let point_at = |vertex_index| {
                vertices.get(vertex_index as usize).copied().ok_or(
                    GeometryError::InvalidTriangleIndex {
                        triangle: triangle_index,
                        vertex: vertex_index,
                    },
                )
            };
            let points = [
                point_at(triangle[0])?,
                point_at(triangle[1])?,
                point_at(triangle[2])?,
            ];
            let first_edge = points[0]
                .vector_to(points[1])?
                .normalized(tolerance)
                .map_err(|_| GeometryError::DegenerateTriangle {
                    triangle: triangle_index,
                })?;
            let second_edge = points[0]
                .vector_to(points[2])?
                .normalized(tolerance)
                .map_err(|_| GeometryError::DegenerateTriangle {
                    triangle: triangle_index,
                })?;
            let sine = first_edge
                .as_vector()
                .cross(second_edge.as_vector())?
                .length()?;
            if sine <= tolerance.angular() {
                return Err(GeometryError::DegenerateTriangle {
                    triangle: triangle_index,
                });
            }
        }

        Ok(Self {
            vertices,
            triangles,
        })
    }

    #[inline]
    pub fn vertices(&self) -> &[Point3] {
        &self.vertices
    }

    #[inline]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// Reverses every face winding without changing mesh vertex or face order.
    pub fn reversed(&self) -> Self {
        let mut triangles = self.triangles.clone();
        for triangle in &mut triangles {
            triangle.swap(1, 2);
        }
        Self {
            vertices: self.vertices.clone(),
            triangles,
        }
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
            closed: self.vertices.len() >= 4
                && self.triangles.len() >= 4
                && boundary_edge_count == 0,
        }
    }

    /// Reorients each manifold-connected face component consistently while
    /// retaining face and vertex order. Exact coincident locations define
    /// adjacency, as they do for [`Self::topology`]. Non-manifold edges do not
    /// impose an ambiguous orientation constraint.
    pub fn unified_face_orientations(&self) -> Result<(Self, usize), GeometryError> {
        let data = self.topology_data();
        let mut neighbors = vec![Vec::<(usize, bool)>::new(); self.triangles.len()];
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

        let mut flipped = vec![None; self.triangles.len()];
        let mut pending = VecDeque::new();
        for root in 0..self.triangles.len() {
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

        let mut triangles = self.triangles.clone();
        let mut flipped_face_count = 0;
        for (triangle, flip) in triangles.iter_mut().zip(flipped) {
            if flip.expect("every face component receives an orientation") {
                triangle.swap(1, 2);
                flipped_face_count += 1;
            }
        }
        Ok((
            Self {
                vertices: self.vertices.clone(),
                triangles,
            },
            flipped_face_count,
        ))
    }

    fn topology_data(&self) -> MeshTopologyData {
        let mut locations = BTreeMap::<[u64; 3], usize>::new();
        let mut topological_vertices = Vec::with_capacity(self.vertices.len());
        for vertex in &self.vertices {
            let key = vertex.to_array().map(canonical_coordinate_bits);
            let location_count = locations.len();
            let id = *locations.entry(key).or_insert(location_count);
            topological_vertices.push(id);
        }

        let mut edges = BTreeMap::<(usize, usize), EdgeIncidence>::new();
        for (face, triangle) in self.triangles.iter().enumerate() {
            let vertices = triangle.map(|index| topological_vertices[index as usize]);
            for [from, to] in [
                [vertices[0], vertices[1]],
                [vertices[1], vertices[2]],
                [vertices[2], vertices[0]],
            ] {
                debug_assert_ne!(from, to, "validated triangle edge collapsed");
                let (edge, forward) = if from < to {
                    ((from, to), true)
                } else {
                    ((to, from), false)
                };
                let incidence = edges.entry(edge).or_default();
                incidence.add_use(EdgeUse { face, forward });
            }
        }

        MeshTopologyData {
            topological_vertex_count: locations.len(),
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
        Self::try_new(vertices, self.triangles.clone(), tolerance)
    }
}

fn canonical_coordinate_bits(coordinate: Real) -> u64 {
    if coordinate == 0.0 {
        0
    } else {
        coordinate.to_bits()
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
