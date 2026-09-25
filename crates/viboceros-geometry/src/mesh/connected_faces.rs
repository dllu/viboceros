//! Face traversal across topological edges with angle or edge-boundary rules.

use super::{GeometryError, Real, TriangleMesh, VecDeque, edge_uses_are_unwelded};

/// Edge boundaries that limit extraction of one mesh part.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshPartBoundary {
    /// Stop only at naked edges, retaining the whole edge-connected piece.
    Naked,
    /// Stop at naked or unwelded edges.
    Unwelded,
    /// Stop at naked, unwelded, or nonmanifold edges.
    UnweldedAndNonManifold,
}

impl TriangleMesh {
    /// Stored faces reachable from `seed` without crossing the chosen edge
    /// boundaries. Coincident raw vertices form a topological edge, but an
    /// unwelded edge has distinct raw vertex indices at both endpoints.
    pub fn mesh_part_faces(
        &self,
        seed: usize,
        boundary: MeshPartBoundary,
    ) -> Result<Vec<usize>, GeometryError> {
        self.validate_seed_face(seed)?;
        let mut adjacency = vec![Vec::new(); self.face_count()];
        for incidence in self.topology_data().edges.values() {
            if incidence.count < 2
                || (boundary == MeshPartBoundary::UnweldedAndNonManifold && incidence.count > 2)
                || (boundary != MeshPartBoundary::Naked && edge_uses_are_unwelded(incidence.uses()))
            {
                continue;
            }
            let mut uses = incidence.uses();
            let first = uses.next().expect("edge has at least two uses").face;
            for edge_use in uses {
                if first != edge_use.face {
                    adjacency[first].push(edge_use.face);
                    adjacency[edge_use.face].push(first);
                }
            }
        }

        let mut visited = vec![false; self.face_count()];
        let mut queue = VecDeque::from([seed]);
        visited[seed] = true;
        while let Some(face) = queue.pop_front() {
            for &neighbor in &adjacency[face] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        Ok(visited
            .into_iter()
            .enumerate()
            .filter_map(|(index, included)| included.then_some(index))
            .collect())
    }

    /// Stored faces reachable from `seed` across edges whose adjacent face
    /// normals meet an inclusive angle comparison. The seed is always included.
    /// Unwelded vertices at identical positions share a topological edge.
    pub fn connected_faces_by_angle(
        &self,
        seed: usize,
        angle_degrees: Real,
        greater_than: bool,
    ) -> Result<Vec<usize>, GeometryError> {
        self.validate_seed_face(seed)?;
        let face_count = self.face_count();
        if !angle_degrees.is_finite() || !(0.0..=180.0).contains(&angle_degrees) {
            return Err(GeometryError::InvalidMeshFaceAngleInterval);
        }

        let normals = self.polygon_face_normals()?;
        let topology = self.topology_data();
        let mut face_edges = vec![Vec::new(); face_count];
        let mut edge_faces = Vec::new();
        for incidence in topology.edges.values() {
            if incidence.count < 2 {
                continue;
            }
            let mut faces = incidence
                .uses()
                .map(|edge_use| edge_use.face)
                .collect::<Vec<_>>();
            faces.dedup();
            if faces.len() < 2 {
                continue;
            }
            let edge_index = edge_faces.len();
            for &face in &faces {
                face_edges[face].push(edge_index);
            }
            edge_faces.push(faces);
        }

        let cosine_limit = angle_degrees.to_radians().cos();
        let mut visited = vec![false; face_count];
        let mut queue = VecDeque::from([seed]);
        visited[seed] = true;
        while let Some(face) = queue.pop_front() {
            for &edge in &face_edges[face] {
                for &neighbor in &edge_faces[edge] {
                    if visited[neighbor] {
                        continue;
                    }
                    let cosine = normals[face]
                        .as_vector()
                        .dot(normals[neighbor].as_vector())?
                        .clamp(-1.0, 1.0);
                    // Both the normalized dot and cosine of the requested
                    // angle round; allow a few ulps at inclusive boundaries.
                    let roundoff = 8.0 * Real::EPSILON;
                    let within_angle = if greater_than {
                        cosine <= cosine_limit + roundoff
                    } else {
                        cosine + roundoff >= cosine_limit
                    };
                    if within_angle {
                        visited[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        Ok(visited
            .into_iter()
            .enumerate()
            .filter_map(|(index, included)| included.then_some(index))
            .collect())
    }

    fn validate_seed_face(&self, seed: usize) -> Result<(), GeometryError> {
        if seed >= self.face_count() {
            return Err(GeometryError::MeshFaceIndexOutOfRange {
                face: seed,
                face_count: self.face_count(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MeshFace, Point3, Tolerance};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn folded_chain() -> TriangleMesh {
        TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(1.0, 1.0, 1.0),
                point(2.0, 1.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(3.0, 1.0, 0.0),
            ],
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([1, 3, 2]),
                MeshFace::Triangle([1, 4, 3]),
                MeshFace::Triangle([6, 7, 8]),
            ],
            Tolerance::default(),
        )
        .unwrap()
    }

    #[test]
    fn traverses_only_edges_meeting_the_inclusive_angle_rule() {
        let mesh = folded_chain();
        assert_eq!(
            mesh.connected_faces_by_angle(0, 0.0, false).unwrap(),
            vec![0, 1]
        );
        assert_eq!(
            mesh.connected_faces_by_angle(0, 90.0, false).unwrap(),
            vec![0, 1, 2]
        );
        assert_eq!(
            mesh.connected_faces_by_angle(0, 90.0, true).unwrap(),
            vec![0]
        );
        assert_eq!(
            mesh.connected_faces_by_angle(1, 90.0, true).unwrap(),
            vec![1, 2]
        );
        assert_eq!(
            mesh.connected_faces_by_angle(0, 0.0, true).unwrap(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn rejects_bad_seed_and_angle() {
        let mesh = folded_chain();
        assert!(matches!(
            mesh.connected_faces_by_angle(4, 0.0, false),
            Err(GeometryError::MeshFaceIndexOutOfRange {
                face: 4,
                face_count: 4
            })
        ));
        for angle in [-1.0, 181.0, Real::NAN] {
            assert!(matches!(
                mesh.connected_faces_by_angle(0, angle, false),
                Err(GeometryError::InvalidMeshFaceAngleInterval)
            ));
        }
    }

    #[test]
    fn traverses_unwelded_and_nonmanifold_topological_edges() {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ],
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([4, 3, 5]),
                MeshFace::Triangle([0, 1, 6]),
            ],
            Tolerance::default(),
        )
        .unwrap();
        assert_eq!(
            mesh.connected_faces_by_angle(0, 0.0, false).unwrap(),
            vec![0, 1]
        );
        assert_eq!(
            mesh.connected_faces_by_angle(0, 90.0, false).unwrap(),
            vec![0, 1, 2]
        );
        assert_eq!(
            mesh.mesh_part_faces(0, MeshPartBoundary::UnweldedAndNonManifold)
                .unwrap(),
            vec![0]
        );
        assert_eq!(
            mesh.mesh_part_faces(0, MeshPartBoundary::Unwelded).unwrap(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn mesh_part_stops_at_unwelded_edges_and_ignores_vertex_only_contacts() {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(-1.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
            ],
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([1, 3, 2]),
                MeshFace::Triangle([4, 6, 5]),
                MeshFace::Triangle([0, 7, 8]),
            ],
            Tolerance::default(),
        )
        .unwrap();
        assert_eq!(
            mesh.mesh_part_faces(0, MeshPartBoundary::UnweldedAndNonManifold)
                .unwrap(),
            vec![0, 1]
        );
        assert_eq!(
            mesh.mesh_part_faces(0, MeshPartBoundary::Naked).unwrap(),
            vec![0, 1, 2]
        );
        assert!(matches!(
            mesh.mesh_part_faces(4, MeshPartBoundary::Naked),
            Err(GeometryError::MeshFaceIndexOutOfRange { .. })
        ));
    }
}
