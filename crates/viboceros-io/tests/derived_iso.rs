use viboceros_geometry::Tolerance;
use viboceros_io::{ThreeDmGeometry, read_3dm_file, write_3dm_file};

#[test]
fn stale_derived_isocurve_flags_are_reclassified_without_changing_geometry() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/derived-iso/stale_flags.3dm");
    let model = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    assert_eq!(model.unsupported_object_count(), 0);
    assert_eq!(model.objects.len(), 2);
    for object in &model.objects {
        let ThreeDmGeometry::Brep(brep) = &object.geometry else {
            panic!("expected B-rep");
        };
        match object.name.as_deref().unwrap() {
            "stale boundary iso" => {
                assert!(brep.is_solid());
                assert_eq!(brep.faces().len(), 6);
                assert_eq!(brep.edges().len(), 12);
                assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap().abs() - 1.).abs() < 1e-12);
            }
            "stale diagonal iso" => {
                assert!(!brep.is_solid());
                assert_eq!(brep.faces().len(), 1);
                assert!((brep.area(Tolerance::DEFAULT).unwrap() - 0.5).abs() < 1e-12);
            }
            name => panic!("unexpected fixture {name}"),
        }
    }
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("reclassified.3dm");
    let preserved = model.clone();
    write_3dm_file(&path, &model).unwrap();
    assert_eq!(model, preserved);
    let decoded = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    assert_eq!(decoded.unsupported_object_count(), 0);
    assert_eq!(decoded.objects.len(), 2);
    for (actual, expected) in decoded.objects.iter().zip(&model.objects) {
        assert_eq!(actual.geometry, expected.geometry);
        assert_eq!(actual.name, expected.name);
    }
}
