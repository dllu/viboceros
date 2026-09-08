use super::*;
use crate::CommandRegistry;
use viboceros_document::SelectionMode;
use viboceros_geometry::Point3;

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
