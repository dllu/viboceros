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
#[path = "support/arc_blend_sources.rs"]
#[allow(dead_code)]
mod sources;
#[path = "support/arc_blend_subcurves.rs"]
mod subcurves;
use viboceros_command::{CommandRegistry, arc_blend::Input};
use viboceros_document::Document;
#[test]
fn temporary_ends_parents_trimming_and_joining_match_native() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in subcurves::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
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
            subcurves::replay(&mut input, &doc, value);
            let registry = CommandRegistry::with_builtins();
            let accepted = value["success"].as_bool().unwrap();
            if accepted {
                registry
                    .execute(&mut doc, &input.arguments().unwrap())
                    .unwrap();
            }
            let (samples, stations) = if accepted {
                sources::samples(&doc, value)
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
                if value["prescribed_case"]["pending_redo"] == true {
                    doc.redo().unwrap();
                    error = error.max(inputs::check(&doc, &value["pending_redo"], false));
                    doc.undo().unwrap();
                }
            }
            let path = directory.path().join("subcurves.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            registry
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(inputs::check(&restored, &value["after"], true));
            println!(
                "ARC_BLEND_SUBCURVE_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_arc_sample_error":samples,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 29);
}

#[test]
fn invalid_temporary_ranges_do_not_consume_sources_or_pending_redo() {
    let data = native::read("arc_blend_subcurve_discovery");
    let value = &data["results"][7]["value"];
    let mut doc = reference_inputs::setup(value, &value["prescribed_case"]);
    doc.clear_selection();
    let first = reference_inputs::guide(&doc, 0);
    let second = reference_inputs::guide(&doc, 1);
    let registry = CommandRegistry::with_builtins();
    for range in ["1,1", "NaN,4", "1,inf", "-1,4", "1,6", "1", "Edge,99,1,4"] {
        let command =
            format!("ArcBlend SubCrv={first},{range} Curve={second} 10,3,0 Trim=Yes Join=Yes");
        assert!(registry.execute(&mut doc, &command).is_err(), "{command}");
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
        assert!(inputs::check(&doc, &value["after"], false) <= 1e-6);
    }
    let mut input = Input::new(doc.tolerance());
    assert!(
        input
            .pick_subcurve(&doc, first, None, [1., 1.], false)
            .is_err()
    );
    assert_eq!(input.reference_count(), 0);
    input
        .pick_subcurve(&doc, first, None, [1., 4.], false)
        .unwrap();
    assert!(
        input
            .pick_subcurve(&doc, second, None, [1., f64::NAN], false)
            .is_err()
    );
    assert_eq!(input.reference_count(), 1);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
}
