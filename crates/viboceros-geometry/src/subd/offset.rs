//! Surface-normal offsets retain the editable control net and native features.
use super::*;
impl SubD {
    /// Move controls along the normalized sum of their distinct limit-sector
    /// normals. Solid outputs bridge naked boundaries to an oppositely oriented
    /// copy of the source. Closed inputs produce the offset skin alone.
    pub fn try_offset(&self, distance: f64, solid: bool) -> Result<Self, GeometryError> {
        Self::from_native(viboceros_opennurbs::subd_offset(
            self.archive(),
            distance,
            solid,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subd_offset_solid_budget_and_nonfinite_distances_leave_source_immutable() {
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
        assert!(source.try_offset(1., true).is_err());
        assert_eq!(source.archive(), archive);
        for distance in [0., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(source.try_offset(distance, false).is_err());
        }
        assert_eq!(source.archive(), archive);
    }
}
