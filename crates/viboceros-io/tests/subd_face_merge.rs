#[path = "support/subd_face_merge.rs"]
mod support;
fn initialize_optional_kernel() {
    #[cfg(feature = "native-smlib")]
    {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
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
            assert_eq!(
                viboceros_smlib::change_curve_degree(&curve, 3, false)
                    .unwrap()
                    .degree(),
                3
            );
        });
    }
}
#[test]
fn native_archives_preserve_definition_ids_limit_points_and_sharpness() {
    initialize_optional_kernel();
    support::native_merge_archives_preserve_definition_ids_and_limit_points();
}
#[test]
fn all_native_merges_match_topology_properties_and_component_identities() {
    initialize_optional_kernel();
    support::face_merge_matches_native_topology_properties_and_component_identities();
}
