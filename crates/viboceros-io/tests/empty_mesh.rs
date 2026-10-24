use viboceros_geometry::{AffineTransform3, Point3, Tolerance, Vector3};
use viboceros_io::{
    ThreeDmDefinition, ThreeDmGeometry, ThreeDmObject, read_3dm_file, read_3dm_file_with_blocks,
    write_3dm_file,
};

#[test]
fn native_empty_mesh_objects_survive_lossless_3dm_round_trips() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/mesh_boolean_empty");
    for (name, expected) in [
        ("boolean_empty_inside", 1),
        ("boolean_empty_later_stage", 2),
    ] {
        let model =
            read_3dm_file(root.join(format!("{name}-result.3dm")), Tolerance::DEFAULT).unwrap();
        assert_eq!(
            model
                .objects
                .iter()
                .filter(|o| matches!(o.geometry, ThreeDmGeometry::EmptyMesh))
                .count(),
            expected
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("empty.3dm");
        write_3dm_file(&path, &model).unwrap();
        let restored = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
        assert_eq!(restored.objects, model.objects);
    }
}

#[test]
fn nested_empty_mesh_definitions_round_trip_without_inventing_bounded_geometry() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/mesh_boolean_empty");
    let mut model = read_3dm_file(
        root.join("boolean_empty_inside-result.3dm"),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let placement =
        AffineTransform3::from_translation(Vector3::try_new(1000., 2000., 3000.).unwrap());
    let reference = || ThreeDmGeometry::InstanceReference {
        definition_index: 0,
        transform: placement,
    };
    let far = Point3::try_new(8000., 9000., 10000.).unwrap();
    model.definitions = vec![
        ThreeDmDefinition {
            name: "empty".into(),
            members: vec![ThreeDmObject::new(ThreeDmGeometry::EmptyMesh, 0)],
        },
        ThreeDmDefinition {
            name: "mixed".into(),
            members: vec![
                ThreeDmObject::new(reference(), 0),
                ThreeDmObject::new(ThreeDmGeometry::Point(far), 0),
            ],
        },
    ];
    model.objects = vec![
        ThreeDmObject::new(reference(), 0),
        ThreeDmObject::new(
            ThreeDmGeometry::InstanceReference {
                definition_index: 1,
                transform: placement,
            },
            0,
        ),
    ];
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("empty-blocks.3dm");
    write_3dm_file(&path, &model).unwrap();
    let restored = read_3dm_file_with_blocks(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(restored.objects, model.objects);
    assert_eq!(restored.definitions, model.definitions);
    let flattened = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    assert_eq!(flattened.objects.len(), 3);
    assert_eq!(
        flattened
            .objects
            .iter()
            .filter(|o| matches!(o.geometry, ThreeDmGeometry::EmptyMesh))
            .count(),
        2
    );
    assert!(
        flattened
            .objects
            .iter()
            .any(|o| o.geometry == ThreeDmGeometry::Point(placement.transform_point(far).unwrap()))
    );
}
