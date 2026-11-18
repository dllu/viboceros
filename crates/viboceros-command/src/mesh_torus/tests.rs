use super::*;
use viboceros_document::SelectionMode;
use viboceros_geometry::Point3;

#[test]
fn mesh_torus_matches_native_vertices_and_faces() {
    let captures: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("rhino_meshes.json")).unwrap();
    for capture in captures {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let mut command = capture["command"].as_str().unwrap().to_owned();
        if command.contains("{guide}") {
            registry
                .execute(&mut document, "Line 0,0,0 0,10,10")
                .unwrap();
            let guide = document.objects().next().unwrap().id();
            command = command.replace("{guide}", &guide.to_string());
        }
        if let Some(points) = capture["points"].as_array() {
            let mut ids = Vec::new();
            for point in points {
                let coords = serde_json::from_value::<[f64; 3]>(point.clone()).unwrap();
                ids.push(
                    document
                        .add_geometry(Geometry::Point(Point3::try_from(coords).unwrap()))
                        .unwrap(),
                );
            }
            document
                .select_objects_direct(ids, SelectionMode::Replace)
                .unwrap();
        }
        let before = document.objects().len();
        registry
            .execute(&mut document, &command)
            .unwrap_or_else(|error| panic!("{command}: {error}"));
        let Geometry::Mesh(mesh) = document.objects().last().unwrap().geometry() else {
            panic!("{command}: expected a polygon mesh");
        };
        let tolerance = capture["tolerance"].as_f64().unwrap_or(1e-10);
        let vertices: Vec<[f64; 3]> = serde_json::from_value(capture["vertices"].clone()).unwrap();
        let faces: Vec<Vec<u32>> = serde_json::from_value(capture["faces"].clone()).unwrap();
        assert_eq!(vertices.len(), mesh.vertices().len(), "{command}");
        assert_eq!(faces.len(), mesh.face_count(), "{command}");
        for (i, (&actual, expected)) in mesh.vertices().iter().zip(vertices).enumerate() {
            let point = Point3::try_from(expected).unwrap();
            assert!(
                actual.distance_to(point).unwrap() < tolerance,
                "{command} vertex {i}: {actual:?} != {point:?}"
            );
        }
        for (i, (actual, expected)) in mesh.faces().iter().zip(faces).enumerate() {
            assert_eq!(actual.indices(), expected, "{command} face {i}");
        }
        assert_eq!(document.undo_label(), Some("MeshTorus"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), before);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().len(), before + 1);
    }
}
