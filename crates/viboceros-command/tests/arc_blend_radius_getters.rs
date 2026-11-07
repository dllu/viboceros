#[path = "support/arc_blend_radius_getters.rs"]
mod getters;
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
fn nested_radius_options_defaults_and_preserved_previews_match_native() {
    let directory = tempfile::tempdir().unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut count = 0;
    for family in getters::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if getters::research(name) {
                continue;
            }
            let id = format!("{family}/{name}");
            let v = &row["value"];
            let mut doc = reference_inputs::setup(v, &v["prescribed_case"]);
            let mut input = Input::new(doc.tolerance());
            getters::replay(&mut input, &doc, v);
            let warning = input.warning().is_some();
            assert_eq!(
                warning,
                v["history"].as_str().unwrap().contains("Unable to achieve"),
                "{id}"
            );
            registry
                .execute(&mut doc, &input.arguments().unwrap())
                .unwrap();
            let samples = radius::independent_samples(&doc, v);
            let mut error = inputs::check(&doc, &v["after"], false);
            doc.undo().unwrap();
            error = error.max(inputs::check(&doc, &v["undo"], false));
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            error = error.max(inputs::check(&doc, &v["redo"], false));
            let path = directory.path().join("getters.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            registry
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(inputs::check(&restored, &v["after"], true));
            println!(
                "ARC_BLEND_GETTER_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_arc_sample_error":samples,"archives":1,"warning":warning})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 9);
}
