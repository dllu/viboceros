#[path = "support/blend_curve.rs"]
#[allow(dead_code)]
mod blend;
#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/arc_blend_handles.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/blend_curve_measurements.rs"]
#[allow(dead_code)]
mod measurements;
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
#[path = "support/blend_curve_edges.rs"]
#[allow(dead_code)]
mod edges;
#[path = "support/blend_curve_closed.rs"]
mod families;

#[path = "support/blend_curve_points.rs"]
#[allow(dead_code)]
mod points;
#[path = "support/blend_curve_portions.rs"]
#[allow(dead_code)]
mod portions;
#[test]
fn whole_closed_sources_match_native_geometry_sources_and_history() {
    replay_workflows();
}
#[test]
fn unused_periodic_padding_is_separate_and_active_coefficients_remain_strict() {
    let data = native::read("blend_curve_closed_holdouts");
    let v = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "closed_periodic_g3")
        .unwrap()["value"];
    let doc = edges::setup(v);
    let mut padded = v["before"].clone();
    let row = padded
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|o| o["attributes"]["name"] == "guide0")
        .unwrap();
    row["curve"]["knots"][0] = serde_json::json!(-100.);
    let (geometry, _, padding) = families::check(&doc, &padded, false);
    assert!(geometry < 1e-6 && padding > 100.);
    let mut changed = v["before"].clone();
    let row = changed
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|o| o["attributes"]["name"] == "guide0")
        .unwrap();
    row["curve"]["knots"][2] = serde_json::json!(2.25);
    let (knot_error, _, _) = families::check(&doc, &changed, false);
    assert!(knot_error > 0.1);
    let mut changed = v["before"].clone();
    let row = changed
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|o| o["attributes"]["name"] == "guide0")
        .unwrap();
    row["curve"]["control_points"][0]["point"][0] = serde_json::json!(-2.9);
    let (control_error, _, _) = families::check(&doc, &changed, false);
    assert!(control_error > 0.01);
    println!(
        "BLEND_CURVE_CLOSED_PADDING_GUARD {}",
        serde_json::json!({"geometry_error_for_unused_padding":geometry,"unused_padding_difference":padding,"active_knot_error":knot_error,"control_error":control_error})
    );
}
fn replay_workflows() {
    let directory = tempfile::tempdir().unwrap();
    let diagnostic = std::cell::Cell::new(false);
    let padding_error = std::cell::Cell::new(0_f64);
    let domain_error = std::cell::Cell::new(0_f64);
    let domain_ratio = std::cell::Cell::new(0_f64);
    let check = |doc: &Document, target: &serde_json::Value, archive| {
        let (error, domain, padding) = families::check(doc, target, archive);
        padding_error.set(padding_error.get().max(padding));
        domain_error.set(domain_error.get().max(domain));
        let ratio = measurements::parameter_domain_ratio(doc, target);
        domain_ratio.set(domain_ratio.get().max(ratio));
        assert!(
            diagnostic.get() || ratio <= 1.,
            "Native fractional domain precision: {domain:e}, ratio={ratio:e}"
        );
        error.max(edges::parent_edges(doc, target))
    };
    let mut count = 0;
    for family in families::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if !families::included(family, name) {
                continue;
            }
            let id = format!("{family}/{name}");
            diagnostic.set(families::domain_diagnostic(family, name));
            padding_error.set(0.);
            domain_error.set(0.);
            domain_ratio.set(0.);
            let value = &row["value"];
            assert_eq!(
                value["input_trace"][0]["errors"],
                serde_json::json!([]),
                "{id}"
            );
            let mut doc = edges::setup(value);
            doc.clear_selection();
            let mut input = Input::with_preferences(doc.tolerance(), edges::preferences(value));
            let rejected = families::replay(&mut input, &doc, value);
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
                "BLEND_CURVE_CLOSED_COMMAND {}",
                serde_json::json!({"id":id,"maximum_unused_outer_knot_difference":padding_error.get(),"maximum_coefficient_and_sample_error":error,"maximum_native_sample_error":samples,"maximum_parameter_domain_error":domain_error.get(),"maximum_parameter_domain_gate_ratio":domain_ratio.get(),"nominal_capture":!diagnostic.get(),"maximum_normalized_derivative_error":derivative_error,"maximum_absolute_derivative_error":before_jets.0.max(after_jets.0),"derivative_stations":before_jets.2+after_jets.2,"native_stations":stations,"archives":1,"rejected_options":rejected,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 45);
}
