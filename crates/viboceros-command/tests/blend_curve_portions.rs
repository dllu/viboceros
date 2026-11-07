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
#[allow(dead_code)]
mod points;
#[path = "support/blend_curve_portions.rs"]
#[allow(dead_code)]
mod portions;
#[test]
fn temporary_curve_portions_match_native_geometry_sources_and_history() {
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
    for family in portions::FAMILIES {
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
            portions::replay(&mut input, &doc, value);
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
                "BLEND_CURVE_PORTION_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_sample_error":samples,"maximum_parameter_domain_error":domain_error.get(),"maximum_normalized_derivative_error":derivative_error,"maximum_absolute_derivative_error":before_jets.0.max(after_jets.0),"derivative_stations":before_jets.2+after_jets.2,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 36);
}

#[test]
fn invalid_bounds_and_bindings_preserve_accepted_sources_and_pending_redo() {
    let value = &native::read("blend_curve_portion_pilot")["results"][0]["value"];
    let mut doc = reference_inputs::setup(value, &value["prescribed_case"]);
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 80,90,100").unwrap();
    doc.undo().unwrap();
    let id = reference_inputs::guide(&doc, 0);
    let other = reference_inputs::guide(&doc, 1);
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let mut input = Input::new(doc.tolerance());
    for range in [[1., 1.], [-1., 4.], [4., 7.], [f64::NAN, 2.]] {
        assert!(input.pick_subcurve(&doc, id, None, range, false).is_err());
        assert_eq!(input.reference_count(), 0);
    }
    input
        .pick_subcurve(&doc, id, None, [1., 4.], false)
        .unwrap();
    input
        .pick_curve(
            &doc,
            other,
            None,
            native::point(&serde_json::json!([10., 3., 0.])),
        )
        .unwrap();
    let preview = input.preview().cloned();
    for option in ["Parameter1=0", "Parameter1=5", "Parameter1=NaN"] {
        assert!(input.option(option).is_err());
        assert_eq!(input.preview(), preview.as_ref());
    }
    for binding in [
        format!("{id},1,1"),
        format!("{id},NaN,2"),
        format!("{id},-1,4"),
        format!("{id},4,7"),
        format!("{id},Edge,999,0,1"),
    ] {
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("BlendCrv SubCrv={binding} Curve={other} 10,3,0")
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
    }
    doc.delete_objects([id]).unwrap();
    assert!(!input.current(&doc));
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    assert!(input.commit(&mut doc).is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), objects);
}
