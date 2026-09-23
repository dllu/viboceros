//! Linear-size mesh concatenation without welding or changing source topology.
use super::*;

impl TriangleMesh {
    /// Concatenates meshes in input order, retaining unused and coincident raw
    /// vertices, polygon winding, n-gons, and each source's internal vertex sharing.
    /// This is not a Boolean union, welding operation, or tolerance-based join.
    pub fn try_append(meshes: &[&Self]) -> Result<Self, GeometryError> {
        if meshes.is_empty() {
            return Err(GeometryError::EmptyMesh);
        }
        let (vertex_count, face_count, triangle_count) = counts(
            meshes
                .iter()
                .map(|mesh| (mesh.vertices.len(), mesh.faces.len(), mesh.triangles.len())),
        )?;
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        let mut triangles = Vec::new();
        let mut ngons = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        faces
            .try_reserve_exact(face_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        triangles
            .try_reserve_exact(triangle_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for mesh in meshes {
            let offset =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            let face_offset = if mesh.ngons.is_empty() {
                0
            } else {
                faces
                    .len()
                    .checked_add(mesh.faces.len())
                    .and_then(|count| count.checked_sub(1))
                    .filter(|&last| u32::try_from(last).is_ok())
                    .ok_or(GeometryError::TooManyMeshFaces)?;
                u32::try_from(faces.len()).map_err(|_| GeometryError::TooManyMeshFaces)?
            };
            vertices.extend_from_slice(&mesh.vertices);
            faces.extend(mesh.faces.iter().map(|face| face.remapped(|i| i + offset)));
            triangles.extend(
                mesh.triangles
                    .iter()
                    .map(|triangle| triangle.map(|i| i + offset)),
            );
            ngons.extend(mesh.ngons.iter().map(|ngon| {
                MeshNgon {
                    vertices: ngon.vertices.iter().map(|&index| index + offset).collect(),
                    faces: ngon
                        .faces
                        .iter()
                        .map(|&index| index + face_offset)
                        .collect(),
                }
            }));
        }
        Ok(Self {
            vertices,
            faces,
            triangles,
            ngons,
        })
    }
}

fn counts(
    mut sources: impl Iterator<Item = (usize, usize, usize)>,
) -> Result<(usize, usize, usize), GeometryError> {
    sources.try_fold((0_usize, 0_usize, 0_usize), |(v, f, t), (a, b, c)| {
        let v = v
            .checked_add(a)
            .filter(|n| {
                n.checked_sub(1)
                    .is_none_or(|last| u32::try_from(last).is_ok())
            })
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let f = f.checked_add(b).ok_or(GeometryError::TooManyMeshFaces)?;
        let t = t.checked_add(c).ok_or(GeometryError::TooManyMeshFaces)?;
        Ok((v, f, t))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad() -> TriangleMesh {
        TriangleMesh::try_new_faces(
            [
                [0., 0., 0.],
                [2., 0., 0.],
                [2., 2., 0.],
                [0., 2., 0.],
                [9., 9., 9.],
            ]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn append_preserves_raw_vertices_winding_quads_and_cached_triangles() {
        let first = quad();
        let second = first.reversed();
        let combined = TriangleMesh::try_append(&[&first, &second]).unwrap();
        assert_eq!(
            combined.vertices(),
            [first.vertices(), second.vertices()].concat()
        );
        assert_eq!(
            combined.faces(),
            [MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([5, 8, 7, 6])]
        );
        assert_eq!(
            combined.triangles(),
            [[0, 1, 2], [0, 2, 3], [5, 8, 7], [5, 7, 6]]
        );
        assert_eq!(combined.area().unwrap(), 8.);
        assert_eq!(TriangleMesh::try_append(&[&first]).unwrap(), first);
        assert!(TriangleMesh::try_append(&[]).is_err());
    }

    #[test]
    fn append_preflights_index_and_allocation_arithmetic() {
        assert!(matches!(
            counts([(usize::MAX, 1, 1), (1, 1, 1)].into_iter()),
            Err(GeometryError::TooManyMeshVertices)
        ));
        assert!(matches!(
            counts([(1, usize::MAX, 1), (1, 1, 1)].into_iter()),
            Err(GeometryError::TooManyMeshFaces)
        ));
        assert!(matches!(
            counts([(1, 1, usize::MAX), (1, 1, 1)].into_iter()),
            Err(GeometryError::TooManyMeshFaces)
        ));
        if let Some(max) = (u32::MAX as usize).checked_add(1) {
            assert_eq!(counts([(max, 1, 1)].into_iter()).unwrap(), (max, 1, 1));
            assert!(counts([(max, 1, 1), (1, 1, 1)].into_iter()).is_err());
        }
    }

    #[test]
    fn many_inputs_have_linear_storage_and_exact_offsets() {
        let source = quad();
        let result = TriangleMesh::try_append(&vec![&source; 10000]).unwrap();
        assert_eq!(result.vertices().len(), 50000);
        assert_eq!(result.faces().len(), 10000);
        assert_eq!(
            result.faces()[9999],
            MeshFace::Quad([49995, 49996, 49997, 49998])
        );
        assert_eq!(result.triangles().len(), 20000);
    }
}
