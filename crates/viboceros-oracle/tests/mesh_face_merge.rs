#[path = "../../viboceros-geometry/tests/support/mesh_face_merge.rs"]
mod support;
#[test]
fn merge_face_components_match_all_48_native_mesh_storage_with_the_active_backend() {
    #[cfg(feature = "native-smlib")]
    {
        // Load the optional kernel in this binary too: the private face-set
        // boundary policy must still control the linked OpenNURBS implementation.
        use viboceros_geometry::{NurbsCurve, Point3};
        let curve = NurbsCurve::try_clamped_uniform(
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 1., 0.).unwrap(),
                Point3::try_new(2., 0., 0.).unwrap(),
            ],
        )
        .unwrap();
        let elevated = viboceros_smlib::change_curve_degree(&curve, 3, false).unwrap();
        assert_eq!(elevated.degree(), 3);
    }
    support::audit_all_native_mesh_storage_and_channels();
}
