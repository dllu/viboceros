use super::*;
use serde_json::Value;
use viboceros_document::{Document, SelectionMode};
use viboceros_geometry::Point3;

#[test]
fn solid_axial_mesh_replays_rhino_vertices_faces_and_history() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("tests/rhino_mesh_primitives.json")).unwrap();
    for case in cases {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let mut command = case["command"].as_str().unwrap().to_owned();
        if command.contains("{guide}") {
            registry.execute(&mut doc, "Line 0,0,0 0,10,10").unwrap();
            command = command.replace("{guide}", &doc.objects().last().unwrap().id().to_string());
        }
        if let Some(points) = case["points"].as_array() {
            let mut picks = Vec::new();
            for point in points {
                let xyz: [f64; 3] = serde_json::from_value(point.clone()).unwrap();
                let id = doc
                    .add_geometry(Geometry::Point(Point3::try_from(xyz).unwrap()))
                    .unwrap();
                picks.push(id);
            }
            doc.select_objects_direct(picks, SelectionMode::Replace)
                .unwrap();
        }
        let before = doc.objects().len();
        registry
            .execute(&mut doc, &command)
            .unwrap_or_else(|e| panic!("{command}: {e}"));
        let Geometry::Mesh(mesh) = doc.objects().last().unwrap().geometry() else {
            panic!("{command}: expected a mesh");
        };
        let vertices: Vec<[f64; 3]> = serde_json::from_value(case["vertices"].clone()).unwrap();
        let faces: Vec<Vec<u32>> = serde_json::from_value(case["faces"].clone()).unwrap();
        let single_precision_cap_centers = case
            .get("single_precision_cap_centers")
            .map_or_else(Vec::<usize>::new, |value| {
                serde_json::from_value(value.clone()).unwrap()
            });
        assert_eq!(mesh.vertices().len(), vertices.len(), "{command}");
        assert_eq!(mesh.faces().len(), faces.len(), "{command}");
        for (idx, (&actual, expected)) in mesh.vertices().iter().zip(vertices).enumerate() {
            let expected = Point3::try_from(expected).unwrap();
            // The captured planar caps save their center at single precision.
            let actual = if single_precision_cap_centers.contains(&idx) {
                Point3::try_from(
                    actual
                        .to_array()
                        .map(|coordinate| f64::from(coordinate as f32)),
                )
                .unwrap()
            } else {
                actual
            };
            assert!(
                actual.distance_to(expected).unwrap() < 1e-9,
                "{command} vertex {idx}: {actual:?} != {expected:?}"
            );
        }
        for (idx, (actual, expected)) in mesh.faces().iter().zip(faces).enumerate() {
            assert_eq!(actual.indices(), &expected, "{command} face {idx}");
        }
        let committed = mesh.clone();
        assert_eq!(doc.undo_label(), command.split_whitespace().next());
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), before);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(
            doc.objects().last().unwrap().geometry(),
            &Geometry::Mesh(committed)
        );
    }
}
