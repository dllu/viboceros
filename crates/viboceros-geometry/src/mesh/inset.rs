//! Face-local mesh insets preserve stored face ordinals and original vertices.
use super::*;
impl TriangleMesh {
    /// Inset stored triangle/quad faces independently, in descending face order.
    /// Existing caps retain their indices. Native incomplete colors and stale
    /// n-gon records are discarded; UVs append zeros and normals are rebuilt.
    pub fn try_inset_faces(&self, indices: &[usize], distance: f64) -> Result<Self, GeometryError> {
        let invalid = || GeometryError::Degenerate {
            context: "mesh inset faces or distance",
        };
        if indices.is_empty()
            || indices.len() > 1_000_000
            || !distance.is_finite()
            || distance <= 0.
        {
            return Err(invalid());
        }
        let selected = indices.iter().copied().collect::<BTreeSet<_>>();
        if selected.iter().any(|&i| i >= self.face_count()) {
            return Err(invalid());
        }
        let added = selected
            .iter()
            .map(|&i| self.faces[i].indices().len())
            .sum::<usize>();
        if self.vertices.len().saturating_add(added) > 4_000_000
            || self.faces.len().saturating_add(added) > 1_000_000
        {
            return Err(invalid());
        }
        let mut vertices = self.vertices.to_vec();
        let mut faces = self.faces.to_vec();
        for &index in selected.iter().rev() {
            let face = self.faces[index];
            let outer = face.indices();
            let n = outer.len();
            let first = u32::try_from(vertices.len()).map_err(|_| invalid())?;
            for j in 0..n {
                let p = self.vertices[outer[j] as usize];
                let a = p
                    .vector_to(self.vertices[outer[(j + n - 1) % n] as usize])?
                    .normalized_nonzero()?
                    .as_vector();
                let b = p
                    .vector_to(self.vertices[outer[(j + 1) % n] as usize])?
                    .normalized_nonzero()?
                    .as_vector();
                let sine = a.cross(b)?.length()?;
                if sine <= 0. {
                    return Err(invalid());
                }
                vertices.push(
                    p.translated(
                        Vector3::try_new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z())?
                            .scaled(distance / sine)?,
                    )?,
                );
            }
            let inner = (0..n).map(|j| first + j as u32).collect::<Vec<_>>();
            faces[index] = match inner.as_slice() {
                [a, b, c] => MeshFace::Triangle([*a, *b, *c]),
                [a, b, c, d] => MeshFace::Quad([*a, *b, *c, *d]),
                _ => unreachable!(),
            };
            for j in 0..n {
                faces.push(MeshFace::Quad([
                    outer[j],
                    outer[(j + 1) % n],
                    inner[(j + 1) % n],
                    inner[j],
                ]));
            }
        }
        let uv = self.texture_coordinates().map(|values| {
            let mut result = values.to_vec();
            result.resize(vertices.len(), [0.; 2]);
            result
        });
        Self::try_from_face_records(vertices, faces)?
            .try_with_texture_coordinates(uv)?
            .retain_boundary_matched_ngons(self.ngons.to_vec())
            .with_recomputed_normal_channel()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inset_mesh_growth_budget_and_nonpositive_distances_reject_without_editing() {
        let source = TriangleMesh::try_from_face_records(
            [[0., 0., 0.], [4., 0., 0.], [0., 3., 0.]]
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            vec![MeshFace::Triangle([0, 1, 2]); 250_001],
        )
        .unwrap();
        assert!(
            source
                .try_inset_faces(&(0..source.face_count()).collect::<Vec<_>>(), 0.3)
                .is_err()
        );
        for distance in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(source.try_inset_faces(&[0], distance).is_err());
        }
        assert_eq!(source.face_count(), 250_001);
        assert_eq!(source.vertices.len(), 3);
    }
}
