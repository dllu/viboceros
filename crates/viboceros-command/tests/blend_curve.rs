#[path = "support/blend_curve.rs"]
mod blend;
#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/arc_blend_handles.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;
#[path = "support/command_file.rs"]
mod replay;
use viboceros_command::{CommandRegistry, blend_curve::Input};
use viboceros_document::Document;
#[test]
fn adjustable_curve_blends_match_native_sources_continuity_and_history() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in blend::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if blend::diagnostic(name) {
                continue;
            }
            let id = format!("{family}/{name}");
            let value = &row["value"];
            assert_eq!(
                value["input_trace"][0]["errors"],
                serde_json::json!([]),
                "{id}"
            );
            let mut doc = reference_inputs::setup(value, &value["prescribed_case"]);
            doc.clear_selection();
            let mut input = Input::new(doc.tolerance());
            blend::replay(&mut input, &doc, value);
            let registry = CommandRegistry::with_builtins();
            let accepted = blend::accepted(value);
            if accepted {
                registry
                    .execute(&mut doc, &input.arguments().unwrap())
                    .unwrap();
            }
            let (samples, stations) = if accepted {
                blend::samples(&doc, value)
            } else {
                (0., 0)
            };
            let mut error = inputs::check(&doc, &value["after"], false);
            if accepted {
                doc.undo().unwrap();
                error = error.max(inputs::check(&doc, &value["undo"], false));
                assert!(!doc.can_undo());
                doc.redo().unwrap();
                error = error.max(inputs::check(&doc, &value["redo"], false));
            } else {
                assert!(!doc.can_undo());
                assert!(doc.can_redo());
                doc.redo().unwrap();
                error = error.max(inputs::check(&doc, &value["pending_redo"], false));
                doc.undo().unwrap();
            }
            let path = directory.path().join("blend.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            registry
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(inputs::check(&restored, &value["after"], true));
            println!(
                "BLEND_CURVE_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_sample_error":samples,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 25);
}

#[test]
fn unit_aware_handles_and_invalid_edits_preserve_sources_and_pending_history() {
    let data = native::read("blend_curve_adjustments");
    let v = &data["results"][0]["value"];
    let mut doc = reference_inputs::setup(v, &v["prescribed_case"]);
    doc.clear_selection();
    let mut input = Input::new(doc.tolerance());
    blend::replay(&mut input, &doc, v);
    let args = input
        .arguments()
        .unwrap()
        .replace("Handle1=3 ", "Handle1=0.3cm ");
    assert!(args.contains("Handle1=0.3cm"));
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, &args).unwrap();
    assert!(inputs::check(&doc, &v["after"], false) <= 1e-6);
    doc.undo().unwrap();
    assert!(!doc.can_undo());
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for bad in [
        "Handle1=0",
        "Handle1=NaN",
        "Handle1=1/0",
        "Parameter1=99",
        "Continuity_1=G4",
    ] {
        let base = input.arguments().unwrap();
        assert!(
            registry
                .execute(&mut doc, &format!("{base} {bad}"))
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.can_redo());
        assert!(!doc.can_undo());
    }
}
