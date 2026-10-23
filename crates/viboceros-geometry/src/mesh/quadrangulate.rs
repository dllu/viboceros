//! Merge welded, consistently oriented triangle pairs into validated quads.

use super::*;

impl TriangleMesh {
    /// Convert triangle pairs with public OpenNURBS selection and angle policy.
    /// Rectangularity is the maximum ratio between a quad's diagonal lengths.
    /// Native conversion determines face slots and n-gon changes; existing
    /// vertices and their stored channels remain unchanged.
    pub fn quadrangulate_triangles(
        &self,
        max_planarity_degrees: Real,
        max_diagonal_ratio: Real,
        _tolerance: Tolerance,
    ) -> Result<(Self, usize), GeometryError> {
        if !max_planarity_degrees.is_finite()
            || !(0.0..=180.0).contains(&max_planarity_degrees)
            || !max_diagonal_ratio.is_finite()
            || max_diagonal_ratio < 1.0
        {
            return Err(GeometryError::InvalidMeshQuadrangulationOptions);
        }
        self.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
        let (mut data, changed) = viboceros_opennurbs::quadrangulate_mesh(
            &self.open_nurbs_data(),
            max_planarity_degrees.to_radians(),
            1. / max_diagonal_ratio,
        )?;
        if changed == 0 {
            return Ok((self.clone(), 0));
        }
        if data.vertices.len() != self.vertices.len()
            || data
                .vertices
                .iter()
                .zip(&self.vertices)
                .any(|(native, point)| *native != point.to_array())
        {
            return Err(viboceros_opennurbs::MeshError(
                "Quad conversion changed vertex storage".into(),
            )
            .into());
        }
        let ngons = std::mem::take(&mut data.ngons)
            .into_iter()
            .map(|g| MeshNgon::from_parts(g.vertices, g.faces))
            .collect();
        let mut result = Self::from_open_nurbs_data(data)?.retain_boundary_matched_ngons(ngons);
        result.vertex_colors = self.vertex_colors.clone();
        result.vertex_channels = self.vertex_channels.clone();
        result.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
        Ok((result, changed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn merges_a_welded_square_and_retains_colors_with_native_ngon_cleanup() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_vertex_colors(Some(vec![[1, 2, 3, 4]; 4]))
        .unwrap()
        .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0, 1])])
        .unwrap();
        let (converted, count) = mesh
            .quadrangulate_triangles(1.0, 2.0, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(converted.faces().len(), 1);
        assert!(matches!(converted.faces()[0], MeshFace::Quad(_)));
        assert_eq!(converted.vertex_colors(), mesh.vertex_colors());
        assert!(converted.ngons().is_empty());
        assert_eq!(
            converted
                .triangulate_quads(Tolerance::DEFAULT)
                .unwrap()
                .0
                .triangles()
                .len(),
            2
        );
    }

    #[test]
    fn does_not_merge_unwelded_or_oppositely_oriented_pairs() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(0.0, 1.0, 0.0),
        ];
        let unwelded = TriangleMesh::try_new(
            vertices.clone(),
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (unchanged, count) = unwelded
            .quadrangulate_triangles(180.0, 100.0, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(unchanged, unwelded);
        let conflict = TriangleMesh::try_new(
            vec![vertices[0], vertices[1], vertices[2], vertices[5]],
            vec![[0, 1, 2], [0, 3, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            conflict
                .quadrangulate_triangles(180.0, 100.0, Tolerance::DEFAULT)
                .unwrap()
                .1,
            0
        );
    }

    #[test]
    fn obeys_planarity_and_diagonal_ratio_limits() {
        let folded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 1.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            folded
                .quadrangulate_triangles(1.0, 10.0, Tolerance::DEFAULT)
                .unwrap()
                .1,
            0
        );
        let skewed = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(10.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            skewed
                .quadrangulate_triangles(1.0, 2.0, Tolerance::DEFAULT)
                .unwrap()
                .1,
            0
        );
        assert_eq!(
            skewed
                .quadrangulate_triangles(1.0, 8.0, Tolerance::DEFAULT)
                .unwrap()
                .1,
            1
        );
    }

    #[test]
    fn invalid_options_leave_the_source_unchanged() {
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
        for (angle, ratio) in [(f64::NAN, 2.0), (-1.0, 2.0), (181.0, 2.0), (1.0, 0.5)] {
            assert_eq!(
                mesh.quadrangulate_triangles(angle, ratio, Tolerance::DEFAULT),
                Err(GeometryError::InvalidMeshQuadrangulationOptions)
            );
        }
    }
}
