use super::*;

/// Threshold used to identify a nonplanar quad. Values are model units and radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NonPlanarQuadCriterion {
    Distance(Real),
    Angle(Real),
    Both { distance: Real, angle: Real },
}

/// Diagonal selection for nonplanar quad triangulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuadSplitMethod {
    ShortestDiagonal,
    LongestDiagonal,
    MinimizeArea,
    MaximizeArea,
    MinimumAngle,
    MaximumAngle,
}

impl TriangleMesh {
    /// Split quads using OpenNURBS' face-plane distance range and both opposite
    /// corner-normal pairs. With both tests enabled, exceeding either threshold
    /// splits the quad. Qualification is independent of the chosen split method.
    pub fn triangulate_nonplanar_quads(
        &self,
        criterion: NonPlanarQuadCriterion,
        method: QuadSplitMethod,
        _tolerance: Tolerance,
    ) -> Result<(Self, usize), GeometryError> {
        let (distance, angle) = match criterion {
            NonPlanarQuadCriterion::Distance(d) => (Some(d), None),
            NonPlanarQuadCriterion::Angle(a) => (None, Some(a)),
            NonPlanarQuadCriterion::Both { distance, angle } => (Some(distance), Some(angle)),
        };
        if distance.is_some_and(|d| !d.is_finite() || d < 0.)
            || angle.is_some_and(|a| !a.is_finite() || !(0. ..=std::f64::consts::PI).contains(&a))
        {
            return Err(GeometryError::InvalidTolerance);
        }
        if !self.faces.iter().any(|f| f.is_quad()) {
            return Ok((self.clone(), 0));
        }
        self.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
        let method = match method {
            QuadSplitMethod::ShortestDiagonal => 1,
            QuadSplitMethod::LongestDiagonal => 2,
            QuadSplitMethod::MinimizeArea => 3,
            QuadSplitMethod::MaximizeArea => 4,
            QuadSplitMethod::MinimumAngle => 5,
            QuadSplitMethod::MaximumAngle => 6,
        };
        let (data, converted) = viboceros_opennurbs::triangulate_nonplanar_mesh(
            &self.open_nurbs_data(),
            distance,
            angle,
            method,
        )?;
        if converted == 0 {
            return Ok((self.clone(), 0));
        }
        Ok((self.decode_native_face_conversion(data)?, converted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_and_split_methods_keep_planar_quads_and_select_nonplanar_ones() {
        let points = [
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
            [0., 1., 0.4],
            [3., 0., 0.],
            [5., 0., 0.],
            [5., 1., 0.],
            [3., 1., 0.],
        ]
        .map(|point| Point3::try_from(point).unwrap());
        let mesh = TriangleMesh::try_new_faces(
            points.to_vec(),
            vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([4, 5, 6, 7])],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_ngons(vec![
            MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0]),
            MeshNgon::from_parts(vec![4, 5, 6, 7], vec![1]),
        ])
        .unwrap();
        let (split, count) = mesh
            .triangulate_nonplanar_quads(
                NonPlanarQuadCriterion::Distance(0.1),
                QuadSplitMethod::ShortestDiagonal,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(split.face_count(), 3);
        assert!(matches!(split.faces()[1], MeshFace::Quad(_)));
        assert_eq!(split.ngons().len(), 1);
        assert_eq!(split.ngons()[0].faces(), &[1]);
        assert_eq!(split.ngons()[0].vertices(), &[4, 5, 6, 7]);
        let (unchanged, count) = mesh
            .triangulate_nonplanar_quads(
                NonPlanarQuadCriterion::Distance(0.5),
                QuadSplitMethod::LongestDiagonal,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(unchanged, mesh);
        for method in [
            QuadSplitMethod::MinimizeArea,
            QuadSplitMethod::MaximizeArea,
            QuadSplitMethod::MinimumAngle,
            QuadSplitMethod::MaximumAngle,
        ] {
            let (result, count) = mesh
                .triangulate_nonplanar_quads(
                    NonPlanarQuadCriterion::Distance(0.1),
                    method,
                    Tolerance::DEFAULT,
                )
                .unwrap();
            assert_eq!(count, 1);
            assert_eq!(result.ngons().len(), 1);
            assert_eq!(result.ngons()[0].faces(), &[1]);
        }
        let (long, count) = mesh
            .triangulate_nonplanar_quads(
                NonPlanarQuadCriterion::Angle(0.1),
                QuadSplitMethod::LongestDiagonal,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_ne!(split.faces()[0], long.faces()[0]);
        let (_, count) = mesh
            .triangulate_nonplanar_quads(
                NonPlanarQuadCriterion::Both {
                    distance: 0.3,
                    angle: 1.0,
                },
                QuadSplitMethod::ShortestDiagonal,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}
