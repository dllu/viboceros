//! Per-face display color and material channels survive native archive editing.
use super::*;
pub use viboceros_opennurbs::SubDFaceProperties;

impl SubD {
    /// One entry per control face, in the same ID-sorted order as `faces()`.
    pub fn face_properties(&self) -> &[SubDFaceProperties] {
        &self.data.face_properties
    }
    /// Set complete per-face properties without editing topology. Every target
    /// must be unique and refer to the original ID-sorted face table.
    pub fn try_with_face_properties(
        &self,
        edits: &[(usize, SubDFaceProperties)],
    ) -> Result<Self, GeometryError> {
        if edits.is_empty() {
            return Ok(self.clone());
        }
        Self::from_native(viboceros_opennurbs::subd_set_face_properties(
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
    fn face_color_material_and_clear_are_exact_across_archive_and_transform() {
        let original = source();
        let properties = SubDFaceProperties {
            color: Some([13, 29, 173, 71]),
            material_channel: u16::MAX,
        };
        let colored = original
            .try_with_face_properties(&[(1, properties)])
            .unwrap();
        assert_eq!(
            original.face_properties(),
            &[SubDFaceProperties::default(); 2]
        );
        assert_eq!(
            colored.face_properties(),
            &[SubDFaceProperties::default(), properties]
        );
        assert_eq!(
            SubD::try_from_archive(colored.archive())
                .unwrap()
                .face_properties(),
            colored.face_properties()
        );
        assert_eq!(colored.face_ids(), original.face_ids());
        assert_eq!(colored.control_points(), original.control_points());
        let moved = colored
            .transformed(AffineTransform3::from_translation(
                Vector3::try_new(7., 11., 17.).unwrap(),
            ))
            .unwrap();
        assert_eq!(moved.face_properties(), colored.face_properties());
        assert_eq!(
            colored
                .try_with_face_properties(&[(1, SubDFaceProperties::default())])
                .unwrap()
                .face_properties(),
            original.face_properties()
        );
    }
    #[test]
    fn late_invalid_duplicate_and_reserved_unset_values_preserve_the_source() {
        let original = source();
        let before = original.clone();
        let value = SubDFaceProperties {
            color: Some([1, 2, 3, 0]),
            material_channel: 17,
        };
        for edits in [
            vec![(0, value), (2, value)],
            vec![(0, value), (0, value)],
            vec![(usize::MAX, value)],
            vec![(
                0,
                SubDFaceProperties {
                    color: Some([255; 4]),
                    ..value
                },
            )],
        ] {
            assert!(original.try_with_face_properties(&edits).is_err());
            assert_eq!(original, before);
        }
    }
}
