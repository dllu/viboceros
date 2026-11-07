#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/arc_blend_handles.rs"]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;
#[path = "support/command_file.rs"]
mod replay;
use viboceros_command::{CommandRegistry, arc_blend::Input};
use viboceros_document::Document;
#[test]
fn handle_adjustments_match_native_geometry_attributes_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let v = &row["value"];
            let id = format!("{family}/{}", row["id"].as_str().unwrap());
            if let Some(reason) = inputs::diagnostic(v) {
                println!(
                    "ARC_BLEND_HANDLE_DIAGNOSTIC {}",
                    serde_json::json!({"id":id,"reason":reason})
                );
                continue;
            }
            let mut doc = reference_inputs::setup(v, &v["prescribed_case"]);
            let mut input = Input::new(doc.tolerance());
            inputs::replay(&mut input, &doc, v);
            registry
                .execute(&mut doc, &input.arguments().unwrap())
                .unwrap();
            let mut error = inputs::check(&doc, &v["after"], false);
            assert_eq!(doc.undo_label(), Some("ArcBlend"));
            doc.undo().unwrap();
            error = error.max(inputs::check(&doc, &v["undo"], false));
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            error = error.max(inputs::check(&doc, &v["redo"], false));
            let path = directory.path().join("handles.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            registry
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(inputs::check(&restored, &v["after"], true));
            println!(
                "ARC_BLEND_HANDLE_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1})
            );
            assert!(error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 26);
}
