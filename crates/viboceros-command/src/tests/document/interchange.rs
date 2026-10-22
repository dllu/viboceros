//! document interchange command regressions.
use super::*;

#[test]
fn imports_and_exports_stl_through_commands() {
    use std::fs;

    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source model.stl");
    let output = directory.path().join("output model.stl");
    fs::write(
            &source,
            "solid test\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid test\n",
        )
        .unwrap();

    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, &format!("ImportStl {}", source.display()))
        .unwrap();
    assert!(matches!(
        document.objects().next().unwrap().geometry(),
        Geometry::Mesh(_)
    ));
    registry
        .execute(
            &mut document,
            &format!("ExportStl Binary {}", output.display()),
        )
        .unwrap();
    let exported = read_stl_file(&output).unwrap();
    assert_eq!(exported.triangles().len(), 1);

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);
    fs::remove_file(source).unwrap();
    fs::remove_file(output).unwrap();
}

#[test]
fn stl_and_step_exports_preserve_planar_brep_holes() {
    use std::fs;

    let rectangle = |min_x, min_y, max_x, max_y| {
        Polyline3::try_new(
            vec![
                Point3::try_new(min_x, min_y, 0.0).unwrap(),
                Point3::try_new(max_x, min_y, 0.0).unwrap(),
                Point3::try_new(max_x, max_y, 0.0).unwrap(),
                Point3::try_new(min_x, max_y, 0.0).unwrap(),
                Point3::try_new(min_x, min_y, 0.0).unwrap(),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let outer = rectangle(-3.0, -3.0, 3.0, 3.0);
    let hole = rectangle(-1.0, -1.0, 1.0, 1.0);
    let brep = Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap();
    let mut document = Document::default();
    document.add_geometry(Geometry::Brep(brep)).unwrap();

    let directory = tempfile::tempdir().unwrap();
    let stl_path = directory.path().join("trimmed-hole.stl");
    let step_path = directory.path().join("trimmed-hole.step");
    let registry = CommandRegistry::with_builtins();

    registry
        .execute(
            &mut document,
            &format!("ExportStl Ascii {}", stl_path.display()),
        )
        .unwrap();
    let stl = read_stl_file(&stl_path).unwrap();
    assert!((stl.area().unwrap() - 32.0).abs() < 1.0e-12);

    registry
        .execute(
            &mut document,
            &format!("ExportStep {}", step_path.display()),
        )
        .unwrap();
    let step = read_step_file(&step_path, Tolerance::DEFAULT).unwrap();
    assert_eq!(step.objects.len(), 1);
    assert!((step.objects[0].mesh.area().unwrap() - 32.0).abs() < 1.0e-12);

    fs::remove_file(stl_path).unwrap();
    fs::remove_file(step_path).unwrap();
}

#[test]
fn imports_and_exports_step_without_polluting_undo_history() {
    use std::fs;

    use monstertruck::modeling::{BoundingBox, Point3 as TruckPoint3, primitive};
    use monstertruck::step::save::{
        CompleteStepDisplay, StepHeaderDescriptor, StepModel as TruckStepModel,
    };

    let cube: monstertruck::modeling::Solid = primitive::cuboid(BoundingBox::from_iter([
        TruckPoint3::new(0.0, 0.0, 0.0),
        TruckPoint3::new(2.0, 3.0, 4.0),
    ]));
    let compressed = cube.compress();
    let step = CompleteStepDisplay::new(
        TruckStepModel::from(&compressed),
        StepHeaderDescriptor {
            organization_system: "Viboceros command test".to_owned(),
            ..Default::default()
        },
    )
    .to_string();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("command model.step");
    let output = directory.path().join("command output.step");
    fs::write(&path, step).unwrap();

    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let message = registry
        .execute(&mut document, &format!("ImportStep {}", path.display()))
        .unwrap();
    assert!(message.contains("1 STEP mesh object"));
    assert!(message.contains("12 triangles"));
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("ImportStep"));
    let Geometry::Mesh(mesh) = document.objects().next().unwrap().geometry() else {
        panic!("expected imported STEP mesh")
    };
    assert_eq!(mesh.bounds().max(), Point3::try_new(2.0, 3.0, 4.0).unwrap());

    let export_message = registry
        .execute(&mut document, &format!("ExportStep {}", output.display()))
        .unwrap();
    assert!(export_message.contains("12 triangles"));
    let exported = read_step_file(&output, document.tolerance()).unwrap();
    // Raw seams in the imported face meshes become separate connected
    // STEP shells. This changes file piece count, not the source document.
    assert_eq!(exported.objects.len(), 6);
    assert!(
        exported
            .objects
            .iter()
            .all(|object| object.mesh.triangles().len() == 2)
    );
    assert_eq!(
        exported
            .objects
            .iter()
            .map(|object| object.mesh.area().unwrap())
            .sum::<f64>(),
        52.0
    );
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("ImportStep"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);
    fs::remove_file(path).unwrap();
    fs::remove_file(output).unwrap();
}

#[test]
fn full_order_3dm_export_reports_actual_counts_without_document_or_history_edits() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("full-order.3dm");
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    let curve = NurbsCurve::try_new(
        1,
        [0.0, 1.0, 3.0, 4.0]
            .map(|x| Point3::try_new(x, 0.0, 0.0).unwrap())
            .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    let id = document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    document.add_group(Some("Pieces".into()), [id]).unwrap();
    registry.execute(&mut document, "Point 7,8,9").unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    let before = document_3dm_model(&document).unwrap();
    let history = (
        document.undo_label().map(str::to_owned),
        document.redo_label().map(str::to_owned),
    );
    let message = registry
        .execute(&mut document, &format!("Export3dm {}", path.display()))
        .unwrap();
    assert!(message.contains("Exported 2 objects in 1 groups"));
    assert!(message.contains("adapted 1 objects with composite curves from 1 source objects"));
    assert_eq!(document_3dm_model(&document).unwrap(), before);
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), vec![id]);
    assert_eq!(document.undo_label(), history.0.as_deref());
    assert_eq!(document.redo_label(), history.1.as_deref());
    let file = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(file.objects.len(), 2);
    assert!(file.objects.iter().all(|o| o.group_indices == vec![0]));
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().len(), 2);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn document_units_are_validated_and_retained_by_3dm_export() {
    use viboceros_geometry::LengthUnitSystem;
    for units in [
        LengthUnitSystem::Inches,
        LengthUnitSystem::Custom {
            name: "eighth-metre".into(),
            meters_per_unit: 0.125,
        },
    ] {
        let mut document = Document::with_units(Tolerance::DEFAULT, units.clone()).unwrap();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.units(), &units);
        let model = document_3dm_model(&document).unwrap();
        assert_eq!(model.units, units);
        assert_eq!(model.objects.len(), 1);
        assert_eq!(
            model.objects[0].geometry,
            ThreeDmGeometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap())
        );
    }
    assert!(
        Document::with_units(
            Tolerance::DEFAULT,
            LengthUnitSystem::Custom {
                name: "bad".into(),
                meters_per_unit: f64::NAN
            }
        )
        .is_err()
    );
    assert_eq!(Document::default().units(), &LengthUnitSystem::Millimeters);
}

#[test]
fn imports_and_exports_3dm_with_layers_and_groups_as_one_undo_step() {
    use std::fs;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("command model.3dm");

    let registry = CommandRegistry::with_builtins();
    let mut source = Document::default();
    registry
        .execute(&mut source, "Layer New Reference")
        .unwrap();
    registry
        .execute(&mut source, "Layer Color 12,34,56 Reference")
        .unwrap();
    registry.execute(&mut source, "Point 1,2,3").unwrap();
    registry
        .execute(&mut source, "Layer Current Default")
        .unwrap();
    registry.execute(&mut source, "Line 0,0,0 4,0,0").unwrap();
    registry.execute(&mut source, "Circle 6,0,0 2").unwrap();
    registry
        .execute(
            &mut source,
            "Arc StartPoint 9,0,0 ThroughPoint 10,1,0 11,0,0",
        )
        .unwrap();
    registry
        .execute(&mut source, "Ellipse 15,0,0 18,0,0 16,2,0")
        .unwrap();
    registry
        .execute(&mut source, "Rectangle 12,0,0 14,2,0")
        .unwrap();
    registry.execute(&mut source, "Polygon 5 20,0,0 2").unwrap();
    registry
        .execute(&mut source, "SrfPt 0,0,0 2,0,0 2,2,1 0,2,1")
        .unwrap();
    let point_cloud_locations = vec![
        Point3::try_new(-3.0, 2.0, 7.0).unwrap(),
        Point3::try_new(8.0, -1.0, 4.0).unwrap(),
        Point3::try_new(-3.0, 2.0, 7.0).unwrap(),
    ];
    source
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_new(point_cloud_locations.clone()).unwrap(),
        ))
        .unwrap();
    registry
        .execute(&mut source, "Box 30,0,0 33,4,0 5")
        .unwrap();
    let source_ids = source
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    source
        .set_objects_color(
            [source_ids[0], source_ids[1]],
            Some(ColorRgb::new(201, 45, 67)),
        )
        .unwrap();
    source
        .select_object(source_ids[1], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut source, "Hide").unwrap();
    source
        .select_object(source_ids[0], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut source, "Lock").unwrap();
    registry
        .execute(&mut source, "Layer Hide Reference")
        .unwrap();
    registry
        .execute(&mut source, "Layer Lock Reference")
        .unwrap();
    source
        .add_group(
            Some("Assembly".to_owned()),
            [source_ids[0], source_ids[1], source_ids[8]],
        )
        .unwrap();
    source
        .add_group(
            Some("Inspection".to_owned()),
            [source_ids[1], source_ids[8]],
        )
        .unwrap();
    source
        .add_group(None, [source_ids[2], source_ids[8]])
        .unwrap();
    source
        .add_empty_group(Some("Empty Fixture".to_owned()))
        .unwrap();
    let export_message = registry
        .execute(&mut source, &format!("Export3dm {}", path.display()))
        .unwrap();
    assert!(export_message.contains("10 objects in 4 groups"));

    let mut imported = Document::default();
    let message = registry
        .execute(&mut imported, &format!("Import3dm {}", path.display()))
        .unwrap();
    assert!(message.contains("10 objects in 4 groups"));
    assert!(message.contains("0 unsupported objects skipped"));
    assert_eq!(imported.objects().len(), 10);
    let imported_ids = imported
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    assert_eq!(imported.groups().len(), 4);
    assert_eq!(
        imported
            .group_by_name("Assembly")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([imported_ids[0], imported_ids[1], imported_ids[8]])
    );
    assert_eq!(
        imported
            .group_by_name("Inspection")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([imported_ids[1], imported_ids[8]])
    );
    assert_eq!(
        imported
            .group_by_name("Group01")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([imported_ids[2], imported_ids[8]])
    );
    assert_eq!(
        imported
            .group_by_name("Empty Fixture")
            .unwrap()
            .members()
            .len(),
        0
    );
    let imported_cloud = imported
        .objects()
        .find_map(|object| match object.geometry() {
            Geometry::PointCloud(cloud) => Some(cloud),
            _ => None,
        })
        .unwrap();
    assert_eq!(imported_cloud.points(), point_cloud_locations);
    assert!(
        imported
            .objects()
            .any(|object| matches!(object.geometry(), Geometry::NurbsSurface(_)))
    );
    let imported_brep = imported
        .objects()
        .find_map(|object| match object.geometry() {
            Geometry::Brep(brep) => Some(brep),
            _ => None,
        })
        .unwrap();
    assert!(imported_brep.is_solid());
    assert_eq!(imported_brep.faces().len(), 6);
    assert_eq!(
        imported
            .objects()
            .filter(|object| matches!(object.geometry(), Geometry::NurbsCurve(_)))
            .count(),
        2
    );
    assert_eq!(
        imported
            .objects()
            .filter(|object| matches!(object.geometry(), Geometry::Arc(_)))
            .count(),
        1
    );
    assert_eq!(
        imported
            .objects()
            .filter(|object| matches!(object.geometry(), Geometry::Polyline(_)))
            .count(),
        2
    );
    assert_eq!(imported.layers().len(), 3);
    let reference = imported.layer_by_name("Reference").unwrap();
    assert_eq!(reference.color(), ColorRgb::new(12, 34, 56));
    assert!(!reference.is_visible());
    assert!(reference.is_locked());
    assert!(imported.layer_by_name("Default (Imported 1)").is_some());
    let point = imported
        .objects()
        .find(|object| matches!(object.geometry(), Geometry::Point(_)))
        .unwrap();
    assert!(point.attributes().is_visible());
    assert!(point.attributes().is_locked());
    let line = imported
        .objects()
        .find(|object| matches!(object.geometry(), Geometry::Line(_)))
        .unwrap();
    assert!(!line.attributes().is_visible());
    assert!(!line.attributes().is_locked());
    for id in [imported_ids[0], imported_ids[1]] {
        let attributes = imported.object(id).unwrap().attributes();
        assert_eq!(attributes.color_source(), ObjectColorSource::Object);
        assert_eq!(attributes.object_color(), ColorRgb::new(201, 45, 67));
    }

    assert_eq!(imported.undo_label(), Some("Import3dm"));
    registry
        .execute(&mut imported, &format!("Import3dm {}", path.display()))
        .unwrap();
    assert_eq!(imported.objects().len(), 20);
    assert!(imported.group_by_name("Assembly (Imported 1)").is_some());
    assert!(imported.group_by_name("Inspection (Imported 1)").is_some());
    assert!(imported.group_by_name("Group01 (Imported 1)").is_some());
    assert!(
        imported
            .group_by_name("Empty Fixture (Imported 1)")
            .is_some()
    );
    registry.execute(&mut imported, "Undo").unwrap();
    assert_eq!(imported.objects().len(), 10);
    assert_eq!(imported.groups().len(), 4);
    registry.execute(&mut imported, "Undo").unwrap();
    assert_eq!(imported.objects().len(), 0);
    assert_eq!(imported.groups().len(), 0);
    assert_eq!(imported.layers().len(), 1);
    fs::remove_file(path).unwrap();
}
