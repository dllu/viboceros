use super::*;

#[test]
fn hidden_members_and_hidden_locked_roots_roundtrip_without_visibility_normalization() {
    let mut model =
        read_3dm_file_with_blocks(fixture("nested_blocks.3dm"), Tolerance::DEFAULT).unwrap();
    model.definitions[0].members[0].visible = false;
    model.objects[0].visible = false;
    model.objects[0].locked = true;
    let file = std::env::temp_dir().join(format!(
        "viboceros-structural-hidden-{}.3dm",
        std::process::id()
    ));
    write_3dm_file(&file, &model).unwrap();
    let read = read_3dm_file_with_blocks(&file, Tolerance::DEFAULT).unwrap();
    assert!(!read.definitions[0].members[0].visible);
    assert!(!read.objects[0].visible);
    assert!(read.objects[0].locked);
    std::fs::remove_file(file).unwrap();
}
fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/instances")
        .join(name)
}
fn near_geometry(a: &ThreeDmGeometry, b: &ThreeDmGeometry) {
    match (a, b) {
        (ThreeDmGeometry::Brep(a), ThreeDmGeometry::Brep(b)) => {
            assert_eq!(a.vertices().len(), b.vertices().len());
            for (a, b) in a.vertices().iter().zip(b.vertices()) {
                assert!(a.point().distance_to(b.point()).unwrap() < 1e-10);
            }
            assert!(
                (a.signed_volume(Tolerance::DEFAULT).unwrap()
                    - b.signed_volume(Tolerance::DEFAULT).unwrap())
                .abs()
                    < 1e-9
            );
        }
        _ => assert_eq!(a, b),
    }
}

#[test]
fn structural_reader_keeps_native_prototypes_nested_references_and_ordinary_objects() {
    let model =
        read_3dm_file_with_blocks(fixture("nested_blocks.3dm"), Tolerance::DEFAULT).unwrap();
    assert_eq!(model.definitions.len(), 2);
    assert_eq!(
        model
            .definitions
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        ["Prototype", "Nested"]
    );
    assert_eq!(model.definitions[0].members.len(), 4);
    assert_eq!(model.definitions[1].members.len(), 1);
    assert_eq!(model.objects.len(), 3);
    assert_eq!(model.expanded_instance_count(), 0);
    assert_eq!(
        model.definitions[0].members[0].geometry,
        ThreeDmGeometry::Point(Point3::try_new(1., 2., 3.).unwrap())
    );
    let ThreeDmGeometry::InstanceReference {
        definition_index,
        transform,
    } = model.definitions[1].members[0].geometry
    else {
        panic!()
    };
    assert_eq!(definition_index, 0);
    assert_eq!(transform.translation().to_array(), [1., 2., 3.]);
    let ThreeDmGeometry::InstanceReference {
        definition_index,
        transform: root,
    } = model.objects[0].geometry
    else {
        panic!()
    };
    assert_eq!(definition_index, 1);
    assert_eq!(
        transform
            .then(root)
            .unwrap()
            .transform_point(Point3::try_new(1., 2., 3.).unwrap())
            .unwrap()
            .to_array(),
        [2., 23., 45.]
    );
    assert_eq!(
        model.objects[2].geometry,
        ThreeDmGeometry::Point(Point3::try_new(99., 98., 97.).unwrap())
    );
}

#[test]
fn native_structural_writer_roundtrips_the_graph_and_flattened_reader_agrees_on_placements() {
    let model =
        read_3dm_file_with_blocks(fixture("nested_blocks.3dm"), Tolerance::DEFAULT).unwrap();
    let file = std::env::temp_dir().join(format!(
        "viboceros-structural-blocks-{}.3dm",
        std::process::id()
    ));
    let report = write_3dm_file(&file, &model).unwrap();
    assert_eq!(report.written_object_count, 3);
    let structural = read_3dm_file_with_blocks(&file, Tolerance::DEFAULT).unwrap();
    assert_eq!(structural.definitions.len(), 2);
    assert_eq!(structural.objects.len(), 3);
    for (actual, expected) in structural.definitions.iter().zip(&model.definitions) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.members.len(), expected.members.len());
        for (actual, expected) in actual.members.iter().zip(&expected.members) {
            near_geometry(&actual.geometry, &expected.geometry);
            assert_eq!(actual.color_source, expected.color_source);
        }
    }
    for (actual, expected) in structural.objects.iter().zip(&model.objects) {
        near_geometry(&actual.geometry, &expected.geometry);
        assert_eq!(actual.group_indices, expected.group_indices);
    }
    let flat = read_3dm_file(&file, Tolerance::DEFAULT).unwrap();
    let original = read_3dm_file(fixture("nested_blocks.3dm"), Tolerance::DEFAULT).unwrap();
    assert_eq!(flat.objects.len(), 9);
    assert_eq!(flat.expanded_instance_count(), 2);
    for (actual, expected) in flat.objects.iter().zip(&original.objects) {
        near_geometry(&actual.geometry, &expected.geometry);
    }
    std::fs::remove_file(file).unwrap();
}

#[test]
fn structural_unit_conversion_scales_prototypes_and_each_translation_once() {
    let model = read_3dm_file_with_blocks_in_units(
        fixture("nested_blocks.3dm"),
        &LengthUnitSystem::Meters,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        model.definitions[0].members[0].geometry,
        ThreeDmGeometry::Point(Point3::try_new(0.001, 0.002, 0.003).unwrap())
    );
    let ThreeDmGeometry::InstanceReference {
        transform: child, ..
    } = model.definitions[1].members[0].geometry
    else {
        panic!()
    };
    let ThreeDmGeometry::InstanceReference {
        transform: root, ..
    } = model.objects[0].geometry
    else {
        panic!()
    };
    assert_eq!(
        child.linear_rows(),
        [[2., 0., 0.], [0., 3., 0.], [0., 0., 4.]]
    );
    assert_eq!(child.translation().to_array(), [0.001, 0.002, 0.003]);
    let point = child
        .then(root)
        .unwrap()
        .transform_point(Point3::try_new(0.001, 0.002, 0.003).unwrap())
        .unwrap();
    assert!(
        point
            .distance_to(Point3::try_new(0.002, 0.023, 0.045).unwrap())
            .unwrap()
            < 1e-14
    );
}

#[test]
fn invalid_reference_graphs_and_singular_or_projective_placements_fail_structural_reading() {
    for file in [
        "missing_block.3dm",
        "cyclic_block.3dm",
        "projective_block.3dm",
        "singular_block.3dm",
    ] {
        assert!(
            read_3dm_file_with_blocks(fixture(file), Tolerance::DEFAULT).is_err(),
            "{file}"
        );
    }
}

#[test]
fn invalid_graph_export_preserves_existing_destination_and_input_model() {
    let mut model =
        read_3dm_file_with_blocks(fixture("nested_blocks.3dm"), Tolerance::DEFAULT).unwrap();
    model.definitions[0].members[0].geometry = ThreeDmGeometry::InstanceReference {
        definition_index: 0,
        transform: AffineTransform3::identity(),
    };
    let before = format!("{model:?}");
    let file = std::env::temp_dir().join(format!(
        "viboceros-invalid-structural-{}.3dm",
        std::process::id()
    ));
    std::fs::write(&file, b"existing model").unwrap();
    assert!(write_3dm_file(&file, &model).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), b"existing model");
    assert_eq!(format!("{model:?}"), before);
    std::fs::remove_file(file).unwrap();
}
