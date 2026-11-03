//! Face insets retain native caps and source properties.
use super::*;
impl SubD {
    /// Inset selected active control faces. Group mode inserts a ring around
    /// their combined boundary; single mode inserts a ring on each face.
    /// Original cap-face ordinals survive both operations.
    pub fn try_inset_faces(
        &self,
        faces: &[usize],
        distance: f64,
        group: bool,
    ) -> Result<Self, GeometryError> {
        match viboceros_opennurbs::subd_inset(self.archive(), faces, distance, group)? {
            Some(data) => Self::from_native(data),
            None => Ok(self.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inset_subd_growth_and_invalid_parameters_leave_source_archive_unchanged() {
        let source = SubD::try_plane_grid(
            crate::Frame3::try_from_directions(
                Point3::try_new(0., 0., 0.).unwrap(),
                Vector3::try_new(1., 0., 0.).unwrap(),
                Vector3::try_new(0., 1., 0.).unwrap(),
                crate::Tolerance::DEFAULT,
            )
            .unwrap(),
            [4., 3.],
            [224, 223],
        )
        .unwrap();
        let archive = source.archive().to_vec();
        assert!(
            source
                .try_inset_faces(&(0..source.faces().len()).collect::<Vec<_>>(), 0.3, false)
                .is_err()
        );
        for distance in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(source.try_inset_faces(&[0], distance, true).is_err());
        }
        assert!(source.try_inset_faces(&[usize::MAX], 0.3, true).is_err());
        assert_eq!(source.archive(), archive);
    }
}
