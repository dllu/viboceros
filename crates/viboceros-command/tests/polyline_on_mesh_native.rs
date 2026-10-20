use viboceros_command::CommandRegistry;
#[path = "support/mesh_polyline.rs"]
mod support;
use support::point;
#[test]
fn native_mesh_polyline_geometry_and_explicit_unresolved_cases() {
    for (op, value) in support::rows() {
        let v = &value;
        let (mut doc, id) = support::setup(v);
        let parent = doc.object(id).unwrap().geometry_snapshot().clone();
        let mut points = Vec::new();
        let mut closed = false;
        let mut canceled = false;
        for a in op["actions"].as_array().unwrap() {
            if a.is_array() {
                points.push(point(a));
            } else {
                match a.as_str().unwrap() {
                    "Undo" => {
                        points.pop();
                    }
                    "Close" => closed = true,
                    "Cancel" => canceled = true,
                    "Enter" => (),
                    other => panic!("{other}"),
                }
            }
        }
        if canceled {
            println!(
                "MESH_POLYLINE_COMMAND {}",
                serde_json::json!({"id":op["id"],"qualified":false,"interactive_only":true})
            );
            continue;
        }
        let input = format!(
            "PolylineOnMesh {} {}",
            points
                .iter()
                .map(|p| p.to_array().map(|x| x.to_string()).join(","))
                .collect::<Vec<_>>()
                .join(" "),
            if closed { "Close" } else { "" }
        );
        let result = CommandRegistry::with_builtins().execute(&mut doc, &input);
        if matches!(op["id"].as_str(), Some("off_mesh" | "disconnected")) {
            assert!(result.is_err(), "{}", op["id"]);
            assert_eq!(doc.objects().count(), 1);
            println!(
                "MESH_POLYLINE_COMMAND {}",
                serde_json::json!({"id":op["id"],"qualified":false,"error":result.unwrap_err().to_string()})
            );
            continue;
        }
        result.unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        let mut error = support::check(&doc, id, v, "after_script");
        assert!(parent.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
        assert_eq!(doc.undo_label(), Some("PolylineOnMesh"));
        doc.undo().unwrap();
        error = error.max(support::check(&doc, id, v, "undo"));
        doc.redo().unwrap();
        error = error.max(support::check(&doc, id, v, "redo"));
        println!(
            "MESH_POLYLINE_COMMAND {}",
            serde_json::json!({"id":op["id"],"qualified":error<1e-6,"maximum_bidirectional_sample_error":error})
        );
        assert!(error < 1e-6, "{} {error:e}", op["id"]);
    }
}
