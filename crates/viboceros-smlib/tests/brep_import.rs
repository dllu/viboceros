#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};
use viboceros_smlib::{BooleanOperation, Solid};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_normal(
        p(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
#[test]
fn rust_solids_import_with_preserved_geometry_and_oriented_volume() {
    let shapes = [
        (
            "box",
            Brep::try_box(frame(), [[0., 10.]; 3], Tolerance::DEFAULT).unwrap(),
        ),
        (
            "cylinder",
            Brep::try_cylinder(frame(), 2., 0., 10., Tolerance::DEFAULT).unwrap(),
        ),
        (
            "sphere",
            Brep::try_surface_grid(
                &viboceros_geometry::NurbsSurface::try_sphere(frame(), 2.).unwrap(),
                &[],
                &[],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
    ];
    let mut failures = Vec::new();
    for (name, brep) in shapes {
        match Solid::from_brep(&brep, Tolerance::DEFAULT) {
            Ok(solid) => {
                let expected = brep.signed_volume(Tolerance::DEFAULT).unwrap();
                let properties = solid.properties(1e-8).unwrap();
                let returned = solid.to_brep(Tolerance::DEFAULT).unwrap();
                assert!(properties.manifold);
                assert!(
                    (properties.volume - expected).abs() < expected * 1e-6,
                    "{name}: {} != {expected}",
                    properties.volume
                );
                assert!(returned.is_solid());
                assert!(
                    (returned.signed_volume(Tolerance::DEFAULT).unwrap() - expected).abs()
                        < expected * 1e-9
                );
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
#[test]
fn imported_operands_support_curved_subtraction_and_stay_unchanged() {
    let a = Brep::try_box(
        frame(),
        [[-5., 5.], [-5., 5.], [0., 10.]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let b = Brep::try_cylinder(frame(), 2., -1., 11., Tolerance::DEFAULT).unwrap();
    let native_a = Solid::from_brep(&a, Tolerance::DEFAULT).unwrap();
    let native_b = Solid::from_brep(&b, Tolerance::DEFAULT).unwrap();
    let result = native_a
        .boolean(&native_b, BooleanOperation::Difference)
        .unwrap()
        .to_brep(Tolerance::DEFAULT)
        .unwrap();
    let expected = 1000. - 40. * std::f64::consts::PI;
    assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - expected).abs() < expected * 1e-9);
    assert!((native_a.properties(1e-8).unwrap().volume - 1000.).abs() < 1e-9);
}

#[test]
fn generated_cad_artifacts_import_including_cavities_and_repeated_cutters() {
    for name in [
        "box",
        "cylinder",
        "sphere",
        "through_hole",
        "pocket",
        "cavity",
        "box_union",
        "box_intersection",
        "box_difference",
        "plate",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../docs/smlib-brep-diagnostics/cad/{name}.3dm"));
        let model = viboceros_io::read_3dm_file(path, Tolerance::DEFAULT).unwrap();
        let viboceros_io::ThreeDmGeometry::Brep(brep) = &model.objects[0].geometry else {
            panic!("B-rep expected")
        };
        let expected = brep.signed_volume(Tolerance::DEFAULT).unwrap();
        let imported =
            Solid::from_brep(brep, Tolerance::DEFAULT).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            imported.material_census().unwrap(),
            if name == "cavity" { [1, 1] } else { [1, 0] },
            "{name} region census"
        );
        let returned = imported
            .to_brep(Tolerance::DEFAULT)
            .unwrap_or_else(|e| panic!("{name} export: {e}"));
        let volume = returned.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (volume - expected).abs() < expected * 1e-9,
            "{name}: {volume} != {expected}"
        );
    }
}

#[test]
fn disconnected_material_shells_remain_distinct_after_import() {
    let first = Brep::try_box(frame(), [[0., 2.]; 3], Tolerance::DEFAULT).unwrap();
    let second =
        Brep::try_box(frame(), [[5., 7.], [0., 2.], [0., 2.]], Tolerance::DEFAULT).unwrap();
    let compound = Brep::try_disjoint_union(vec![first, second], Tolerance::DEFAULT).unwrap();
    let imported = Solid::from_brep(&compound, Tolerance::DEFAULT).unwrap();
    let returned = imported.to_brep(Tolerance::DEFAULT).unwrap();
    assert_eq!(imported.material_census().unwrap(), [2, 0]);
    assert_eq!(returned.edge_connected_face_components().len(), 2);
    assert!((returned.signed_volume(Tolerance::DEFAULT).unwrap() - 16.).abs() < 1e-9);
    assert!((imported.properties(1e-8).unwrap().volume - 16.).abs() < 1e-9);
    let parts = imported.material_parts().unwrap();
    assert_eq!(parts.len(), 2);
    for part in parts {
        assert!(
            (part
                .to_brep(Tolerance::DEFAULT)
                .unwrap()
                .signed_volume(Tolerance::DEFAULT)
                .unwrap()
                - 8.)
                .abs()
                < 1e-9
        );
    }
}

#[test]
fn imported_cavity_and_disconnected_solids_classify_followup_cutters() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/smlib-brep-diagnostics/cad/cavity.3dm");
    let model = viboceros_io::read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    let viboceros_io::ThreeDmGeometry::Brep(brep) = &model.objects[0].geometry else {
        panic!()
    };
    let imported = Solid::from_brep(brep, Tolerance::DEFAULT).unwrap();
    let cutter = Solid::box_solid(p(4.5, 4.5, 4.5), [1.; 3]).unwrap();
    let unchanged = imported
        .boolean(&cutter, BooleanOperation::Difference)
        .unwrap()
        .to_brep(Tolerance::DEFAULT)
        .unwrap();
    assert!(
        (unchanged.signed_volume(Tolerance::DEFAULT).unwrap()
            - brep.signed_volume(Tolerance::DEFAULT).unwrap())
        .abs()
            < 1e-9
    );
    let first = Brep::try_box(frame(), [[0., 2.]; 3], Tolerance::DEFAULT).unwrap();
    let second =
        Brep::try_box(frame(), [[5., 7.], [0., 2.], [0., 2.]], Tolerance::DEFAULT).unwrap();
    let compound = Brep::try_disjoint_union(vec![first, second], Tolerance::DEFAULT).unwrap();
    let imported = Solid::from_brep(&compound, Tolerance::DEFAULT).unwrap();
    let cutter = Solid::box_solid(p(6., -1., -1.), [2., 4., 4.]).unwrap();
    let result = imported
        .boolean(&cutter, BooleanOperation::Difference)
        .unwrap()
        .to_brep(Tolerance::DEFAULT)
        .unwrap();
    assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - 12.).abs() < 1e-9);
}

#[test]
fn nested_island_regions_and_repeated_followup_operations_remain_valid() {
    let outer = Brep::try_box(frame(), [[0., 10.]; 3], Tolerance::DEFAULT).unwrap();
    let mut cavity = Brep::try_box(frame(), [[2., 8.]; 3], Tolerance::DEFAULT).unwrap();
    cavity.reverse_orientation();
    let island = Brep::try_box(frame(), [[4., 6.]; 3], Tolerance::DEFAULT).unwrap();
    let nested = Brep::try_disjoint_union(vec![outer, cavity, island], Tolerance::DEFAULT).unwrap();
    let expected = 1000. - 216. + 8.;
    for _ in 0..8 {
        let imported = Solid::from_brep(&nested, Tolerance::DEFAULT).unwrap();
        assert_eq!(imported.material_census().unwrap(), [2, 1]);
        let cutter = Solid::box_solid(p(4.5, 4.5, 4.5), [1.; 3]).unwrap();
        let result = imported
            .boolean(&cutter, BooleanOperation::Difference)
            .unwrap()
            .to_brep(Tolerance::DEFAULT)
            .unwrap();
        assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - (expected - 1.)).abs() < 1e-9);
        assert!(
            (imported
                .to_brep(Tolerance::DEFAULT)
                .unwrap()
                .signed_volume(Tolerance::DEFAULT)
                .unwrap()
                - expected)
                .abs()
                < 1e-9
        );
    }
}

#[test]
fn overlapping_component_shells_fail_before_becoming_a_native_solid() {
    let a = Brep::try_box(frame(), [[0., 3.]; 3], Tolerance::DEFAULT).unwrap();
    let b = Brep::try_box(frame(), [[2., 5.]; 3], Tolerance::DEFAULT).unwrap();
    let source = Brep::try_disjoint_union(vec![a, b], Tolerance::DEFAULT).unwrap();
    assert!(Solid::from_brep(&source, Tolerance::DEFAULT).is_err());
    let valid = Brep::try_box(frame(), [[0., 2.]; 3], Tolerance::DEFAULT).unwrap();
    assert_eq!(
        Solid::from_brep(&valid, Tolerance::DEFAULT)
            .unwrap()
            .material_census()
            .unwrap(),
        [1, 0]
    );
}

#[test]
fn complete_subtraction_returns_an_empty_owned_result() {
    let source = Brep::try_cylinder(frame(), 2., 0., 5., Tolerance::DEFAULT).unwrap();
    let a = Solid::from_brep(&source, Tolerance::DEFAULT).unwrap();
    let b = Solid::box_solid(p(-3., -3., -1.), [6., 6., 7.]).unwrap();
    let empty = a.boolean(&b, BooleanOperation::Difference).unwrap();
    assert!(empty.is_empty().unwrap());
    assert!(empty.to_brep(Tolerance::DEFAULT).is_err());
    assert!(!a.is_empty().unwrap());
}

#[test]
fn open_shells_and_touching_components_are_rejected_without_poisoning_context() {
    let solid = Brep::try_box(frame(), [[0., 2.]; 3], Tolerance::DEFAULT).unwrap();
    let open = solid.sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    assert!(Solid::from_brep(&open, Tolerance::DEFAULT).is_err());
    let touching =
        Brep::try_box(frame(), [[2., 4.], [0., 2.], [0., 2.]], Tolerance::DEFAULT).unwrap();
    let compound =
        Brep::try_disjoint_union(vec![solid.clone(), touching], Tolerance::DEFAULT).unwrap();
    assert!(Solid::from_brep(&compound, Tolerance::DEFAULT).is_err());
    assert!(
        (Solid::from_brep(&solid, Tolerance::DEFAULT)
            .unwrap()
            .properties(1e-8)
            .unwrap()
            .volume
            - 8.)
            .abs()
            < 1e-9
    );
}

#[test]
fn inconsistent_shell_sense_is_rejected() {
    let mut source = Brep::try_box(frame(), [[0., 2.]; 3], Tolerance::DEFAULT).unwrap();
    source.reverse_orientation();
    assert!(Solid::from_brep(&source, Tolerance::DEFAULT).is_err());
    let outer = Brep::try_box(frame(), [[0., 10.]; 3], Tolerance::DEFAULT).unwrap();
    let inner = Brep::try_box(frame(), [[2., 8.]; 3], Tolerance::DEFAULT).unwrap();
    let invalid = Brep::try_disjoint_union(vec![outer, inner], Tolerance::DEFAULT).unwrap();
    assert!(Solid::from_brep(&invalid, Tolerance::DEFAULT).is_err());
}
