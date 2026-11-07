#[path = "support/arc_blend_radius_general.rs"]
mod general;
#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/arc_blend_handles.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/arc_blend_radius.rs"]
#[allow(dead_code)]
mod radius;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;
#[path = "support/command_file.rs"]
mod replay;
use viboceros_command::{CommandRegistry, arc_blend::Input};
use viboceros_document::Document;
#[test]
fn radius_targets_match_native_arcs_samples_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut matched = 0;
    for family in general::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let id = format!("{family}/{name}");
            let v = &row["value"];
            let mut doc = reference_inputs::setup(v, &v["prescribed_case"]);
            let mut input = Input::new(doc.tolerance());
            radius::replay(&mut input, &doc, v);
            let warning = general::final_warning(v);
            assert_eq!(input.warning().is_some(), warning, "{id}");
            let message = registry
                .execute(&mut doc, &input.arguments().unwrap())
                .unwrap();
            assert_eq!(message.contains("Unable to achieve"), warning);
            let representation_diagnostic = general::representation_diagnostic(name);
            let sample_error = if representation_diagnostic {
                general::coincident_circle_error(&doc, v)
            } else {
                radius::independent_samples(&doc, v)
            };
            let mut error = inputs::check(&doc, &v["after"], false);
            doc.undo().unwrap();
            error = error.max(inputs::check(&doc, &v["undo"], false));
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            error = error.max(inputs::check(&doc, &v["redo"], false));
            let path = directory.path().join("radius.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            registry
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(inputs::check(&restored, &v["after"], true));
            if representation_diagnostic {
                assert!(general::coincident_circle_error(&restored, v) <= 1e-6);
            }
            println!(
                "ARC_BLEND_RADIUS_GENERAL_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_arc_sample_error":sample_error,"archives":1,"warning":warning,"representation_diagnostic":representation_diagnostic})
            );
            assert!(
                (representation_diagnostic || error <= 1e-6) && sample_error <= 1e-6,
                "{id}: {error:e}, {sample_error:e}"
            );
            matched += 1;
        }
    }
    assert_eq!(matched, 79);
}
