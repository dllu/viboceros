//! Three-dimensional UVW coordinates indexed by control-face corner.
use super::*;
pub use viboceros_opennurbs::SubDTextureCoordinateType;

impl SubD {
    /// The archive's source of evaluated texture coordinates.
    pub fn texture_coordinate_type(&self) -> SubDTextureCoordinateType {
        self.data.texture_coordinate_type
    }
    /// Custom UVW values in the same ID-sorted face and oriented corner order as
    /// `faces()`. An absent face array has no custom points; an absent corner
    /// represents an unset/nonfinite native value retained in the archive.
    pub fn face_texture_points(&self) -> &[Option<Vec<Option<[f64; 3]>>>] {
        &self.data.face_texture_points
    }
    /// Replace complete corner arrays on unique original face ordinals. Setting
    /// any array activates FromFaceTexturePoints; clearing arrays leaves the
    /// texture source unchanged. Invalid edits reject the complete operation.
    pub fn try_with_face_texture_points(
        &self,
        edits: &[(usize, Option<Vec<[f64; 3]>>)],
    ) -> Result<Self, GeometryError> {
        if edits.is_empty() {
            return Ok(self.clone());
        }
        Self::from_native(viboceros_opennurbs::subd_set_face_texture_points(
            self.archive(),
            edits,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> SubD {
        SubD::try_from_polygons(
            [
                [0., 0., 0.],
                [2., 0., 0.],
                [2., 2., 0.],
                [0., 2., 0.],
                [4., 0., 0.],
                [4., 2., 0.],
            ]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
            vec![vec![0, 1, 2, 3], vec![1, 4, 5, 2]],
        )
        .unwrap()
    }
    #[test]
    fn uvw_points_are_independent_of_geometry_and_survive_archive_and_transform() {
        let original = source();
        let points = vec![[0., 0., 0.25], [3., 0., 0.5], [3., 2., 0.75], [0., 2., 1.]];
        let textured = original
            .try_with_face_texture_points(&[(1, Some(points.clone()))])
            .unwrap();
        assert_eq!(
            textured.texture_coordinate_type(),
            SubDTextureCoordinateType::FromFaceTexturePoints
        );
        assert_eq!(
            textured.face_texture_points(),
            &[None, Some(points.into_iter().map(Some).collect())]
        );
        assert_eq!(original.face_texture_points(), &[None, None]);
        assert_eq!(textured.faces(), original.faces());
        assert_eq!(textured.control_points(), original.control_points());
        let restored = SubD::try_from_archive(textured.archive()).unwrap();
        assert_eq!(
            restored.face_texture_points(),
            textured.face_texture_points()
        );
        let moved = textured
            .transformed(AffineTransform3::from_translation(
                Vector3::try_new(7., 11., 17.).unwrap(),
            ))
            .unwrap();
        assert_eq!(moved.face_texture_points(), textured.face_texture_points());
        assert_eq!(
            moved.texture_coordinate_type(),
            textured.texture_coordinate_type()
        );
        let cleared = textured.try_with_face_texture_points(&[(1, None)]).unwrap();
        assert_eq!(cleared.face_texture_points(), &[None, None]);
        assert_eq!(
            cleared.texture_coordinate_type(),
            textured.texture_coordinate_type()
        );
    }
    #[test]
    fn invalid_late_counts_indices_duplicates_and_nonfinite_coordinates_are_atomic() {
        let original = source();
        let before = original.clone();
        let points = Some(vec![[0., 0., 0.]; 4]);
        for edits in [
            vec![(0, points.clone()), (2, None)],
            vec![(0, points.clone()), (1, Some(vec![[0.; 3]; 3]))],
            vec![(0, points.clone()), (0, None)],
            vec![(usize::MAX, None)],
            vec![(0, Some(vec![]))],
            vec![(0, Some(vec![[f64::NAN, 0., 0.]; 4]))],
            vec![(0, Some(vec![[f64::INFINITY, 0., 0.]; 4]))],
        ] {
            assert!(original.try_with_face_texture_points(&edits).is_err());
            assert_eq!(original, before);
        }
        assert_eq!(
            original.try_with_face_texture_points(&[]).unwrap(),
            original
        );
    }
}
