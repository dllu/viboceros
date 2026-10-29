#[path = "support/subd_allocation.rs"]
mod allocation;
#[path = "support/subd_face_custom.rs"]
mod custom;
#[path = "support/subd_face_properties.rs"]
mod properties;
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

#[test]
fn native_face_colors_and_sequential_merges_preserve_geometry_and_properties() {
    initialize_optional_kernel();
    properties::audit();
}

#[test]
fn allocating_subdivision_after_native_merges_preserves_component_ids_and_geometry() {
    initialize_optional_kernel();
    allocation::audit();
}

#[test]
fn native_material_channels_and_custom_texture_points_survive_edits_and_readback() {
    initialize_optional_kernel();
    custom::audit();
}
