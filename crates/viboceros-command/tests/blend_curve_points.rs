#[path = "support/blend_curve.rs"]
#[allow(dead_code)]
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
#[path = "support/blend_curve_points.rs"]
mod points;
#[test]
fn point_endpoints_match_native_geometry_sources_and_history() {
    replay_workflows();
}
fn replay_workflows() {
    let directory = tempfile::tempdir().unwrap();
    let domain_error = std::cell::Cell::new(0_f64);
    let check = |doc: &Document, target: &serde_json::Value, archive| {
        let (error, domain) = blend::check_high(doc, target, archive);
        domain_error.set(domain_error.get().max(domain));
        assert!(
            domain <= 2e-6,
            "native approximate length domain: {domain:e}"
        );
        error
    };
    let mut count = 0;
    for family in points::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if points::diagnostic(name) {
                continue;
            }
            let id = format!("{family}/{name}");
            domain_error.set(0.);
            let value = &row["value"];
            assert_eq!(
                value["input_trace"][0]["errors"],
                serde_json::json!([]),
                "{id}"
            );
            let mut doc = reference_inputs::setup(value, &value["prescribed_case"]);
            doc.clear_selection();
            let mut input = Input::with_preferences(doc.tolerance(), points::preferences(value));
            blend::replay(&mut input, &doc, value);
            let identities = points::identities(&doc);
            let before_jets = blend::derivatives(&doc, &value["before"]);
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
            points::check_identities(&doc, &identities, &value["after"]);
            let after_jets = blend::derivatives(&doc, &value["after"]);
            let derivative_error = before_jets.1.max(after_jets.1);
            assert!(
                derivative_error <= 1e-6,
                "{id}: normalized derivative error {derivative_error:e}"
            );
            let mut error = check(&doc, &value["after"], false);
            if accepted {
                doc.undo().unwrap();
                error = error.max(check(&doc, &value["undo"], false));
                assert!(!doc.can_undo());
                doc.redo().unwrap();
                error = error.max(check(&doc, &value["redo"], false));
            } else {
                assert!(!doc.can_undo());
                assert!(doc.can_redo());
                doc.redo().unwrap();
                error = error.max(check(&doc, &value["pending_redo"], false));
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
            error = error.max(check(&restored, &value["after"], true));
            println!(
                "BLEND_CURVE_POINT_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_sample_error":samples,"maximum_parameter_domain_error":domain_error.get(),"maximum_normalized_derivative_error":derivative_error,"maximum_absolute_derivative_error":before_jets.0.max(after_jets.0),"derivative_stations":before_jets.2+after_jets.2,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 41);
}

#[test]
fn invalid_point_options_coincident_contacts_and_second_points_are_atomic() {
    let v = native::read("blend_curve_point_pilot")["results"][0]["value"].clone();
    let mut doc = reference_inputs::setup(&v, &v["prescribed_case"]);
    doc.clear_selection();
    let mut input = Input::new(doc.tolerance());
    input
        .pick_point(native::point(&serde_json::json!([10., 3., 0.])))
        .unwrap();
    assert!(
        input
            .pick_point(native::point(&serde_json::json!([0., 0., 0.])))
            .is_err()
    );
    assert_eq!(input.reference_count(), 1);
    input
        .pick_curve(
            &doc,
            reference_inputs::guide(&doc, 0),
            None,
            native::point(&serde_json::json!([0., 0., 0.])),
        )
        .unwrap();
    assert_eq!(input.continuities(), [0, 2]);
    let before = input.preview().unwrap().clone();
    for option in ["Continuity_1=G1", "Parameter1=0", "Flip1=Yes"] {
        assert!(input.option(option).is_err());
        assert_eq!(input.preview(), Some(&before));
    }
    input
        .select_handle(viboceros_command::blend_curve::Handle::Start)
        .unwrap();
    assert!(
        input
            .adjust_point(native::point(&serde_json::json!([0., 0., 0.])))
            .is_err()
    );
    assert_eq!(input.preview(), Some(&before));
    assert!(!doc.can_undo());
    let registry = CommandRegistry::with_builtins();
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    for command in [
        "BlendCrv Point=0,0,0 Point=1,0,0",
        "BlendCrv Point=NaN,0,0",
        "BlendCrv Point=0,0,0",
    ] {
        assert!(registry.execute(&mut doc, command).is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), objects);
        assert!(!doc.can_undo());
    }
}
