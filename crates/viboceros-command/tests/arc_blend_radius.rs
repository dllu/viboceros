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
    let (mut matched, mut diagnostics) = (0, 0);
    for family in radius::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let id = format!("{family}/{name}");
            let v = &row["value"];
            if let Some(reason) = radius::diagnostic(name) {
                println!(
                    "ARC_BLEND_RADIUS_DIAGNOSTIC {}",
                    serde_json::json!({"id":id,"reason":reason})
                );
                diagnostics += 1;
                continue;
            }
            let mut doc = reference_inputs::setup(v, &v["prescribed_case"]);
            let mut input = Input::new(doc.tolerance());
            radius::replay(&mut input, &doc, v);
            let warning = v["history"]
                .as_str()
                .unwrap()
                .contains("Unable to achieve requested radius difference");
            assert_eq!(input.warning().is_some(), warning, "{id}");
            let message = registry
                .execute(&mut doc, &input.arguments().unwrap())
                .unwrap();
            assert_eq!(message.contains("Unable to achieve"), warning);
            let sample_error = radius::independent_samples(&doc, v);
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
            println!(
                "ARC_BLEND_RADIUS_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_arc_sample_error":sample_error,"archives":1,"warning":warning})
            );
            assert!(
                error <= 1e-6 && sample_error <= 1e-6,
                "{id}: {error:e}, {sample_error:e}"
            );
            matched += 1;
        }
    }
    assert_eq!((matched, diagnostics), (18, 23));
}
#[test]
fn unsupported_and_invalid_radius_changes_preserve_the_preview_and_document() {
    let data = native::read("arc_blend_radius_targets");
    for row in data["results"].as_array().unwrap() {
        let v = &row["value"];
        let doc = reference_inputs::setup(v, &v["prescribed_case"]);
        let mut input = Input::new(doc.tolerance());
        for a in &v["prescribed_case"]["actions"].as_array().unwrap()[..2] {
            input
                .pick_curve(
                    &doc,
                    reference_inputs::guide(&doc, a["curve"].as_u64().unwrap() as usize),
                    None,
                    native::point(&a["pick"]),
                )
                .unwrap();
        }
        let before = input.preview().unwrap().clone();
        let arguments = input.arguments().unwrap();
        for word in ["RadiusDifference=NaN", "RadiusDifference=inf", "Ratio=-1"] {
            assert!(input.option(word).is_err());
            assert_eq!(input.preview(), Some(&before));
            assert_eq!(input.arguments().unwrap(), arguments);
        }
        if radius::diagnostic(row["id"].as_str().unwrap()).is_some() {
            assert!(input.option("RadiusDifference=0").is_err());
            assert_eq!(input.preview(), Some(&before));
            assert_eq!(input.arguments().unwrap(), arguments);
        }
        assert!(!doc.can_undo());
    }
}
