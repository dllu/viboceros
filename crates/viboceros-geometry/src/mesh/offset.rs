//! Vertex displacement and boundary walls for mesh offsets.

use super::*;
use crate::Vector3;

/// Direction used for every mesh vertex. Coincident raw vertices share a
/// topological offset direction in `VertexNormals` mode.
#[derive(Clone, Copy, Debug)]
pub enum MeshOffsetDirection {
    VertexNormals,
    AverageNormals { fallback: UnitVector3 },
    Vector(UnitVector3),
}

impl TriangleMesh {
    /// Offset an indexed mesh, optionally including both displaced skins and
    /// walls along naked topological edges. Positive distance follows the
    /// stored polygon winding. A solid offset of a closed mesh has two shells.
    pub fn offset_mesh(
        &self,
        distance: Real,
        direction: MeshOffsetDirection,
        both_sides: bool,
        solid: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !distance.is_finite() || distance == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "mesh offset distance",
            });
        }
        let directions = self.offset_directions(direction)?;
        let make_skin = |signed_distance: Real| -> Result<Self, GeometryError> {
            let mut vertices = Vec::with_capacity(self.vertices.len());
            for (&point, normal) in self.vertices.iter().zip(&directions) {
                vertices.push(point.translated(normal.scaled(signed_distance)?)?);
            }
            let mut mesh = Self::try_new_faces(vertices, self.faces.clone(), tolerance)?;
            mesh.vertex_colors = self.vertex_colors.clone();
            mesh.ngons = self.ngons.clone();
            Ok(mesh)
        };
        let positive = (both_sides || distance > 0.0)
            .then(|| make_skin(distance.abs()))
            .transpose()?;
        let negative = (both_sides || distance < 0.0)
            .then(|| make_skin(-distance.abs()))
            .transpose()?;
        if !solid {
            return if both_sides {
                Self::try_append(&[negative.as_ref().unwrap(), positive.as_ref().unwrap()])
            } else if distance > 0.0 {
                Ok(positive.unwrap())
            } else {
                Ok(negative.unwrap())
            };
        }
        let topology = self.topology();
        if !topology.is_manifold() || !topology.is_oriented() {
            return Err(GeometryError::Degenerate {
                context: "mesh offset solid topology",
            });
        }
        let (bottom, top) = if both_sides {
            (negative.unwrap(), positive.unwrap())
        } else if distance < 0.0 {
            (negative.unwrap(), self.clone())
        } else {
            (self.clone(), positive.unwrap())
        };
        self.offset_solid_between(&bottom, &top, tolerance)
    }

    fn offset_directions(
        &self,
        direction: MeshOffsetDirection,
    ) -> Result<Vec<Vector3>, GeometryError> {
        if let MeshOffsetDirection::Vector(vector) = direction {
            return Ok(vec![vector.as_vector(); self.vertices.len()]);
        }
        // Follow the public ON_Mesh::ComputeVertexNormals and OffsetMesh
        // algorithm in third_party/opennurbs/opennurbs_mesh.cpp: compute
        // float vertex normals, then average them for each exact-location
        // topology vertex before displacing all its raw copies.
        let normals = self.polygon_face_normals()?;
        let mut sums = vec![[0.0_f32; 3]; self.vertices.len()];
        for (face, normal) in self.faces.iter().zip(normals).rev() {
            for &index in face.indices() {
                let index = index as usize;
                for (sum, coordinate) in sums[index].iter_mut().zip(normal.as_vector().to_array()) {
                    *sum += coordinate as f32;
                }
            }
        }
        let world_z = Vector3::try_new(0.0, 0.0, 1.0)?;
        let raw_normals = sums
            .iter()
            .map(|&sum| {
                let vector = Vector3::try_new(sum[0] as Real, sum[1] as Real, sum[2] as Real)?;
                let normalized = vector
                    .normalized_nonzero()
                    .map(UnitVector3::as_vector)
                    .unwrap_or(world_z);
                Vector3::try_from(
                    normalized
                        .to_array()
                        .map(|coordinate| coordinate as f32 as Real),
                )
            })
            .collect::<Result<Vec<Vector3>, GeometryError>>()?;
        match direction {
            MeshOffsetDirection::VertexNormals => {
                let topology = self.topology_data();
                let mut sums = vec![[0.0_f64; 3]; topology.topological_vertex_count];
                for (normal, &vertex) in raw_normals.iter().zip(&topology.topological_vertices) {
                    for (sum, component) in sums[vertex].iter_mut().zip(normal.to_array()) {
                        *sum += component;
                    }
                }
                let directions = sums
                    .into_iter()
                    .map(|sum| {
                        Vector3::try_from(sum).map(|vector| {
                            vector
                                .normalized_nonzero()
                                .map(UnitVector3::as_vector)
                                .unwrap_or(Vector3::try_new(0.0, 0.0, 0.0).unwrap())
                        })
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                Ok(topology
                    .topological_vertices
                    .iter()
                    .map(|&index| directions[index])
                    .collect())
            }
            MeshOffsetDirection::AverageNormals { fallback } => {
                let mut sum = [0.0_f64; 3];
                for normal in &raw_normals {
                    for (total, component) in sum.iter_mut().zip(normal.to_array()) {
                        *total += component;
                    }
                }
                let average = if sum.iter().map(|value| value.abs()).fold(0.0, Real::max)
                    <= 32.0 * Real::EPSILON * raw_normals.len() as Real
                {
                    fallback.as_vector()
                } else {
                    Vector3::try_from(sum)?.normalized_nonzero()?.as_vector()
                };
                Ok(vec![average; self.vertices.len()])
            }
            MeshOffsetDirection::Vector(_) => unreachable!(),
        }
    }

    fn offset_solid_between(
        &self,
        bottom: &Self,
        top: &Self,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let count = self.vertices.len();
        let offset = u32::try_from(count).map_err(|_| GeometryError::TooManyMeshVertices)?;
        count
            .checked_mul(2)
            .and_then(|total| total.checked_sub(1))
            .filter(|&last| u32::try_from(last).is_ok())
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let (_, naked) = self.topology_with_boundary();
        self.faces
            .len()
            .checked_mul(2)
            .and_then(|count| count.checked_add(naked.len()))
            .and_then(|count| count.checked_sub(1))
            .filter(|&last| u32::try_from(last).is_ok())
            .ok_or(GeometryError::TooManyMeshFaces)?;
        let mut vertices = bottom.vertices.clone();
        vertices.extend_from_slice(&top.vertices);
        let mut faces = self
            .faces
            .iter()
            .copied()
            .map(MeshFace::reversed)
            .collect::<Vec<_>>();
        faces.extend(
            self.faces
                .iter()
                .copied()
                .map(|face| face.remapped(|i| i + offset)),
        );
        for edge in naked {
            let indices = self.faces[edge.face].indices();
            let [a, b] = indices
                .iter()
                .copied()
                .zip(indices.iter().copied().cycle().skip(1))
                .take(indices.len())
                .find(|&(a, b)| [a, b] == edge.vertices || [b, a] == edge.vertices)
                .map(|(a, b)| [a, b])
                .expect("naked edge comes from its source face");
            faces.push(MeshFace::Quad([a, b, b + offset, a + offset]));
        }
        let mut mesh = Self::try_new_faces(vertices, faces, tolerance)?;
        if let (Some(bottom_colors), Some(top_colors)) = (&bottom.vertex_colors, &top.vertex_colors)
        {
            let mut colors = bottom_colors.clone();
            colors.extend_from_slice(top_colors);
            mesh.vertex_colors = Some(colors);
        }
        let face_offset =
            u32::try_from(self.faces.len()).map_err(|_| GeometryError::TooManyMeshFaces)?;
        let mut ngons = Vec::with_capacity(self.ngons.len() * 2);
        for source in &self.ngons {
            for shift in [0, face_offset] {
                let faces = source.faces.iter().map(|&face| face + shift).collect();
                if let Some(ngon) = mesh.ngon_from_faces(faces) {
                    ngons.push(ngon);
                }
            }
        }
        mesh.ngons = ngons;
        if !mesh.topology().is_solid() {
            return Err(GeometryError::Degenerate {
                context: "mesh offset solid closure",
            });
        }
        Ok(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> TriangleMesh {
        TriangleMesh::try_new(
            [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]]
                .into_iter()
                .map(|point| Point3::try_from(point).unwrap())
                .collect(),
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn normal_offset_and_solid_shell_have_expected_geometry() {
        let source = triangle();
        let offset = source
            .offset_mesh(
                2.0,
                MeshOffsetDirection::VertexNormals,
                false,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(offset.face_count(), 1);
        assert!(offset.vertices().iter().all(|point| point.z() == 2.0));
        for distance in [2.0, -2.0] {
            let shell = source
                .offset_mesh(
                    distance,
                    MeshOffsetDirection::VertexNormals,
                    false,
                    true,
                    Tolerance::DEFAULT,
                )
                .unwrap();
            assert_eq!(shell.face_count(), 5);
            assert!(shell.topology().is_solid());
            assert!((shell.signed_volume().unwrap() - 1.0).abs() < 1e-12);
        }
        let both = source
            .offset_mesh(
                2.0,
                MeshOffsetDirection::VertexNormals,
                true,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert!(both.topology().is_solid());
        assert!((both.signed_volume().unwrap() - 2.0).abs() < 1e-12);
    }

    #[test]
    fn average_opposing_normals_uses_supplied_fallback() {
        let source = triangle();
        let opposed = TriangleMesh::try_append(&[&source, &source.reversed()]).unwrap();
        let fallback = UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
        let output = opposed
            .offset_mesh(
                3.0,
                MeshOffsetDirection::AverageNormals { fallback },
                false,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
        for (before, after) in opposed.vertices().iter().zip(output.vertices()) {
            assert_eq!(after.x() - before.x(), 3.0);
            assert_eq!(after.y(), before.y());
            assert_eq!(after.z(), before.z());
        }
    }

    #[test]
    fn solid_offset_rejects_collapsed_boundary_walls() {
        let source = triangle();
        let direction = UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
        assert!(
            source
                .offset_mesh(
                    1.0,
                    MeshOffsetDirection::Vector(direction),
                    false,
                    true,
                    Tolerance::DEFAULT
                )
                .is_err()
        );
    }

    #[test]
    fn closed_mesh_solid_offset_has_two_oriented_shells() {
        let mesh = TriangleMesh::try_new(
            [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
                .into_iter()
                .map(|point| Point3::try_from(point).unwrap())
                .collect(),
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let shell = mesh
            .offset_mesh(
                0.1,
                MeshOffsetDirection::VertexNormals,
                false,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert!(shell.topology().is_solid());
        assert_eq!(shell.disjoint_pieces().len(), 2);
        assert!(shell.signed_volume().unwrap() > 0.0);
    }

    #[test]
    fn colored_ngon_survives_open_and_solid_offset() {
        let mesh = TriangleMesh::try_new(
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
                .into_iter()
                .map(|point| Point3::try_from(point).unwrap())
                .collect(),
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_vertex_colors(Some(vec![[10, 20, 30, 0]; 4]))
        .unwrap()
        .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0, 1])])
        .unwrap();
        let open = mesh
            .offset_mesh(
                1.0,
                MeshOffsetDirection::VertexNormals,
                false,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(open.vertex_colors(), mesh.vertex_colors());
        assert_eq!(open.ngons(), mesh.ngons());
        let shell = mesh
            .offset_mesh(
                1.0,
                MeshOffsetDirection::VertexNormals,
                false,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(shell.vertex_colors().unwrap().len(), 8);
        assert_eq!(shell.ngons().len(), 2);
    }

    #[test]
    fn solid_offset_keeps_unwelded_crease_joined() {
        let mesh = TriangleMesh::try_new(
            [
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 0., 1.],
            ]
            .into_iter()
            .map(|point| Point3::try_from(point).unwrap())
            .collect(),
            vec![[0, 1, 2], [4, 3, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let offset = mesh
            .offset_mesh(
                0.25,
                MeshOffsetDirection::VertexNormals,
                false,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(offset.vertices()[0], offset.vertices()[3]);
        assert_eq!(offset.vertices()[1], offset.vertices()[4]);
        let shell = mesh
            .offset_mesh(
                0.25,
                MeshOffsetDirection::VertexNormals,
                false,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert!(shell.topology().is_solid());
    }
}
