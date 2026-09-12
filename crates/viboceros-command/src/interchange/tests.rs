use super::*;
use crate::CommandRegistry;
use viboceros_document::SelectionMode;
use viboceros_geometry::Point3;

#[test]
fn file_commands_preserve_repeated_spaces_in_quoted_and_unquoted_paths() {
    let directory = tempfile::tempdir().unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut source = Document::default();
    source.add_geometry(triangle(0.0)).unwrap();
    let before = format!("{source:?}");
    for (export, import, extension) in [
        ("ExportStl Ascii", "ImportStl", "stl"),
        ("ExportStl Binary", "ImportStl", "stl"),
        ("ExportStep", "ImportStep", "step"),
        ("ExportStep", "ImportStep Native=Yes", "step"),
        ("Export3dm", "Import3dm", "3dm"),
    ] {
        for quoted in [false, true] {
            let path = directory
                .path()
                .join(format!("two  spaces {quoted}.{extension}"));
            let argument = if quoted {
                format!("\"{}\"", path.display())
            } else {
                path.display().to_string()
            };
            registry
                .execute(&mut source, &format!("{export} {argument}"))
                .unwrap();
            assert!(path.is_file(), "export changed the filename");
            assert_eq!(format!("{source:?}"), before);
            let mut target = Document::default();
            registry
                .execute(&mut target, &format!("{import} {argument}"))
                .unwrap();
            assert_eq!(target.objects().len(), 1);
            let bounds = target.objects().next().unwrap().geometry().bounds();
            assert_eq!(bounds.max(), Point3::try_new(0.5, 0.5, 0.0).unwrap());
        }
    }
}

#[test]
fn malformed_filename_quotes_fail_before_document_history_changes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    document.add_geometry(triangle(0.0)).unwrap();
    document.undo().unwrap();
    let before = format!("{document:?}");
    for command in [
        "ImportStl",
        "ExportStl",
        "ImportStep",
        "ImportStep Native=Yes",
        "ExportStep",
        "Import3dm",
        "Export3dm",
    ] {
        for argument in ["\"unfinished", "\"\"", "\"file\" trailing"] {
            assert!(matches!(
                registry.execute(&mut document, &format!("{command} {argument}")),
                Err(CommandError::Usage(_))
            ));
            assert_eq!(format!("{document:?}"), before);
        }
    }
}

#[test]
fn native_step_import_keeps_cavity_shells_in_one_document_object() {
    use monstertruck::modeling::{BoundingBox, Point3 as TruckPoint, Shell, Solid, primitive};
    use monstertruck::step::save::{CompleteStepDisplay, StepHeaderDescriptor, StepModel};
    let cube = |radius: f64| -> Solid {
        primitive::cuboid(BoundingBox::from_iter([
            TruckPoint::new(-radius, -radius, -radius),
            TruckPoint::new(radius, radius, radius),
        ]))
    };
    let outer = cube(5.);
    let inner = cube(1.);
    let cavity = Shell::from(
        inner.boundaries()[0]
            .iter()
            .map(|face| face.inverse())
            .collect::<Vec<_>>(),
    );
    let solid = Solid::new(vec![outer.boundaries()[0].clone(), cavity]).compress();
    let text = CompleteStepDisplay::new(StepModel::from(&solid), StepHeaderDescriptor::default())
        .to_string();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cavity.step");
    std::fs::write(&path, text).unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(
            &mut document,
            &format!("ImportStep Native=Yes {}", path.display()),
        )
        .unwrap();
    assert_eq!(document.objects().len(), 1);
    let Geometry::Brep(brep) = document.objects().next().unwrap().geometry() else {
        panic!("lost native B-rep")
    };
    assert_eq!(
        (
            brep.vertices().len(),
            brep.edges().len(),
            brep.faces().len()
        ),
        (16, 24, 12)
    );
    assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 992.).abs() < 1e-9);
}

#[test]
fn native_step_import_converts_units_and_replays_as_editable_breps() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native  part.step");
    let registry = CommandRegistry::with_builtins();
    let mut source =
        Document::with_units(Tolerance::DEFAULT, viboceros_io::LengthUnitSystem::Inches).unwrap();
    source.add_geometry(triangle(1.)).unwrap();
    registry
        .execute(&mut source, &format!("ExportStep \"{}\"", path.display()))
        .unwrap();
    let mut target = Document::with_units(
        Tolerance::DEFAULT,
        viboceros_io::LengthUnitSystem::Millimeters,
    )
    .unwrap();
    let message = registry
        .execute(
            &mut target,
            &format!("ImportStep Native=Yes \"{}\"", path.display()),
        )
        .unwrap();
    assert!(message.contains("1 native planar STEP object"));
    assert_eq!(target.undo_label(), Some("ImportStep"));
    let object = target.objects().next().unwrap();
    let Geometry::Brep(brep) = object.geometry() else {
        panic!("native import produced a mesh")
    };
    assert_eq!(
        (
            brep.vertices().len(),
            brep.edges().len(),
            brep.faces().len()
        ),
        (3, 3, 1)
    );
    assert!((brep.area(Tolerance::DEFAULT).unwrap() - 0.125 * 25.4 * 25.4).abs() < 1e-9);
    assert!(brep.vertices().iter().any(|v| {
        v.point()
            .is_near(Point3::try_new(25.4, 0., 0.).unwrap(), Tolerance::DEFAULT)
    }));
    let snapshot = format!("{:?}", target.objects().collect::<Vec<_>>());
    registry.execute(&mut target, "Undo").unwrap();
    assert_eq!(target.objects().len(), 0);
    registry.execute(&mut target, "Redo").unwrap();
    assert_eq!(
        format!("{:?}", target.objects().collect::<Vec<_>>()),
        snapshot
    );
}

#[test]
fn failed_native_step_import_preserves_document_and_redo_history() {
    let registry = CommandRegistry::with_builtins();
    let directory = tempfile::tempdir().unwrap();
    let invalid = directory.path().join("invalid.step");
    std::fs::write(&invalid, "not a STEP file").unwrap();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 1,2,3").unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    let before = format!("{document:?}");
    for command in [
        "ImportStep Native=Yes".to_owned(),
        format!("ImportStep Native=Yes \"{}\"", invalid.display()),
    ] {
        assert!(registry.execute(&mut document, &command).is_err());
        assert_eq!(format!("{document:?}"), before);
    }
}

#[test]
fn file_import_name_collisions_preserve_assignments_and_replay_exactly() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("named parts.3dm");
    let mut source_object = ThreeDmObject::new(
        ThreeDmGeometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()),
        1,
    );
    source_object.group_indices = vec![1, 0];
    source_object.name = Some("imported point".into());
    source_object.object_color = [40, 50, 60];
    source_object.color_source = ThreeDmColorSource::Object;
    let source = ThreeDmModel::new(
        vec![
            ThreeDmLayer {
                name: "PART".into(),
                color: [1, 2, 3],
                visible: true,
                locked: false,
            },
            ThreeDmLayer {
                name: "DETAIL".into(),
                color: [4, 5, 6],
                visible: false,
                locked: true,
            },
        ],
        vec![
            ThreeDmGroup {
                name: "Assembly".into(),
            },
            ThreeDmGroup {
                name: "Fixture".into(),
            },
        ],
        vec![source_object],
    );
    write_3dm_file(&path, &source).unwrap();
    let mut document = Document::default();
    for name in ["Part", "Part (Imported 1)", "detail"] {
        document.add_layer(name, ColorRgb::new(10, 20, 30)).unwrap();
    }
    for name in ["Assembly", "Assembly (Imported 1)", "fixture"] {
        document.add_empty_group(Some(name.into())).unwrap();
    }
    let original = document.add_geometry(triangle(0.0)).unwrap();
    document
        .select_object(original, SelectionMode::Replace)
        .unwrap();
    let before = document_3dm_model(&document).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut document, &format!("Import3dm {}", path.display()))
        .unwrap();
    assert!(document.layer_by_name("PART (Imported 2)").is_some());
    let layer = document.layer_by_name("DETAIL (Imported 1)").unwrap();
    assert!(!layer.is_visible());
    assert!(layer.is_locked());
    assert_eq!(layer.color(), ColorRgb::new(4, 5, 6));
    let object = document
        .objects()
        .find(|object| object.attributes().name() == Some("imported point"))
        .unwrap();
    let imported_id = object.id();
    assert_eq!(object.attributes().layer_id(), layer.id());
    assert_eq!(
        object.attributes().object_color(),
        ColorRgb::new(40, 50, 60)
    );
    assert_eq!(
        object.attributes().color_source(),
        ObjectColorSource::Object
    );
    assert_eq!(
        object
            .group_ids()
            .iter()
            .map(|id| document.group(*id).unwrap().name().unwrap())
            .collect::<Vec<_>>(),
        ["Fixture", "Assembly (Imported 2)"]
    );
    let after = document_3dm_model(&document).unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document_3dm_model(&document).unwrap(), before);
    assert!(document.object(imported_id).is_none());
    assert!(document.is_selected(original));
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document_3dm_model(&document).unwrap(), after);
    assert!(document.object(imported_id).is_some());
    // A later import must initialize its index from the changed document,
    // including names assigned by the first import and restored by redo.
    registry
        .execute(&mut document, &format!("Import3dm {}", path.display()))
        .unwrap();
    assert!(document.layer_by_name("PART (Imported 3)").is_some());
    assert!(document.layer_by_name("DETAIL (Imported 2)").is_some());
    assert!(document.group_by_name("Assembly (Imported 3)").is_some());
    assert!(document.group_by_name("Fixture (Imported 1)").is_some());
    let repeated = document_3dm_model(&document).unwrap();
    assert_eq!(repeated.objects.len(), 3);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document_3dm_model(&document).unwrap(), after);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document_3dm_model(&document).unwrap(), repeated);
}

#[test]
fn generated_group_names_scan_each_candidate_only_once() {
    let used = (1..=1000)
        .map(|i| format!("Group{i:02}"))
        .collect::<BTreeSet<_>>();
    let attempts = std::cell::Cell::new(0);
    let mut numbers = (1_u64..=u64::MAX).inspect(|_| attempts.set(attempts.get() + 1));
    for i in 1001..=5000 {
        assert_eq!(
            next_serialized_group_name(&used, &mut numbers),
            format!("Group{i:02}")
        );
    }
    assert_eq!(attempts.get(), 5000);
    assert_eq!(used.len(), 1000);
}

#[test]
fn export_group_names_reserve_later_named_groups_and_preserve_membership_order() {
    let mut document = Document::default();
    let object = document.add_geometry(triangle(0.0)).unwrap();
    let first = document.add_empty_group(None).unwrap();
    let named = document.add_empty_group(Some("Group01".into())).unwrap();
    let second = document.add_empty_group(None).unwrap();
    document.add_empty_group(Some("Group03".into())).unwrap();
    document.add_empty_group(Some("Group100".into())).unwrap();
    document
        .set_object_group_memberships(object, [second, named, first])
        .unwrap();
    let before = format!("{document:?}");
    let model = document_3dm_model(&document).unwrap();
    assert_eq!(
        model
            .groups
            .iter()
            .map(|g| g.name.as_str())
            .collect::<Vec<_>>(),
        ["Group02", "Group01", "Group04", "Group03", "Group100"]
    );
    assert_eq!(model.objects[0].group_indices, [2, 1, 0]);
    assert_eq!(format!("{document:?}"), before);
    assert_eq!(document_3dm_model(&document).unwrap(), model);
}

fn triangle(x: f64) -> Geometry {
    Geometry::Mesh(
        TriangleMesh::try_new(
            vec![
                Point3::try_new(x, 0.0, 0.0).unwrap(),
                Point3::try_new(x + 0.5, 0.0, 0.0).unwrap(),
                Point3::try_new(x, 0.5, 0.0).unwrap(),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    )
}

fn exported_origins(document: &Document) -> Vec<f64> {
    let mesh = combined_document_mesh(document).unwrap();
    (0..mesh.triangles().len())
        .map(|i| mesh.triangle_points(i).unwrap()[0].x())
        .collect()
}

#[test]
fn mesh_export_visibility_matches_isolation_and_layer_state() {
    let mut document = Document::default();
    let selected = document.add_geometry(triangle(0.0)).unwrap();
    document.add_geometry(triangle(2.0)).unwrap();
    let hidden = document.add_geometry(triangle(4.0)).unwrap();
    document.set_objects_visibility([hidden], false).unwrap();
    let locked = document.add_geometry(triangle(6.0)).unwrap();
    document.set_objects_locked([locked], true).unwrap();
    let hidden_layer = document
        .add_layer("hidden", ColorRgb::new(1, 2, 3))
        .unwrap();
    let locked_layer = document
        .add_layer("locked", ColorRgb::new(4, 5, 6))
        .unwrap();
    document
        .add_geometry_with_attributes(triangle(8.0), ObjectAttributes::on_layer(hidden_layer))
        .unwrap();
    document
        .add_geometry_with_attributes(triangle(10.0), ObjectAttributes::on_layer(locked_layer))
        .unwrap();
    document.set_layer_visibility(hidden_layer, false).unwrap();
    document.set_layer_locked(locked_layer, true).unwrap();
    document
        .select_object(selected, SelectionMode::Replace)
        .unwrap();
    let registry = CommandRegistry::with_builtins();

    for (command, expected) in [
        (None, vec![0.0, 2.0, 6.0, 10.0]),
        (Some("Isolate"), vec![0.0, 6.0, 10.0]),
        (Some("Unisolate"), vec![0.0, 2.0, 6.0, 10.0]),
        (Some("Undo"), vec![0.0, 6.0, 10.0]),
        (Some("Redo"), vec![0.0, 2.0, 6.0, 10.0]),
    ] {
        if let Some(command) = command {
            registry.execute(&mut document, command).unwrap();
        }
        let before = format!("{document:?}");
        assert_eq!(exported_origins(&document), expected);
        // 3DM retains the entire model, including both forms of hidden geometry.
        let model = document_3dm_model(&document).unwrap();
        assert_eq!(model.objects.len(), 6);
        assert!(!model.objects[2].visible);
        assert!(model.objects[3].locked);
        assert!(!model.layers[model.objects[4].layer_index].visible);
        assert!(model.layers[model.objects[5].layer_index].locked);
        assert_eq!(format!("{document:?}"), before);
    }
}

#[test]
fn no_visible_mesh_export_keeps_existing_destinations_and_document_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let mut document = Document::default();
    let id = document.add_geometry(triangle(0.0)).unwrap();
    document.set_objects_visibility([id], false).unwrap();
    let registry = CommandRegistry::with_builtins();
    let before = format!("{document:?}");
    for (command, name) in [
        ("ExportStl Ascii", "ascii.stl"),
        ("ExportStl Binary", "binary.stl"),
        ("ExportStep", "mesh.step"),
    ] {
        let path = directory.path().join(name);
        std::fs::write(&path, b"original").unwrap();
        assert!(matches!(
            registry.execute(&mut document, &format!("{command} {}", path.display())),
            Err(CommandError::NoMeshToExport)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert_eq!(format!("{document:?}"), before);
    }
}
