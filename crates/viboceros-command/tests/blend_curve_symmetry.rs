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
#[path = "support/blend_curve_symmetry_families.rs"]
mod families;
#[path = "support/blend_curve_points.rs"]
#[allow(dead_code)]
mod points;
#[path = "support/blend_curve_portions.rs"]
#[allow(dead_code)]
mod portions;
#[test]
fn shift_shape_edits_match_native_geometry_sources_and_history() {
    replay_workflows();
}
fn replay_workflows() {
    let directory = tempfile::tempdir().unwrap();
    let domain_error = std::cell::Cell::new(0_f64);
    let domain_ratio = std::cell::Cell::new(0_f64);
    let tight_length_error = std::cell::Cell::new(0_f64);
    let check = |doc: &Document, target: &serde_json::Value, archive| {
        let (error, domain) = blend::check_high(doc, target, archive);
        domain_error.set(domain_error.get().max(domain));
        let ratio = measurements::parameter_domain_ratio(doc, target);
        let length_error = measurements::tight_length_error(doc, target);
        domain_ratio.set(domain_ratio.get().max(ratio));
        tight_length_error.set(tight_length_error.get().max(length_error));
        assert!(
            ratio <= 1.,
            "Native desired fractional length precision: {domain:e}, ratio={ratio:e}"
        );
        assert!(
            length_error <= 1e-7,
            "independent tight Native length: {length_error:e}"
        );
        error
    };
    let mut count = 0;
    for family in families::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if points::diagnostic(name) {
                continue;
            }
            let id = format!("{family}/{name}");
            domain_error.set(0.);
            domain_ratio.set(0.);
            tight_length_error.set(0.);
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
                "BLEND_CURVE_SYMMETRY_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_sample_error":samples,"maximum_parameter_domain_error":domain_error.get(),"maximum_parameter_domain_gate_ratio":domain_ratio.get(),"maximum_tight_length_error":tight_length_error.get(),"maximum_normalized_derivative_error":derivative_error,"maximum_absolute_derivative_error":before_jets.0.max(after_jets.0),"derivative_stations":before_jets.2+after_jets.2,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 33);
}

#[test]
fn paired_edits_are_atomic_transient_and_distinct_from_contact_edits() {
    use viboceros_command::blend_curve::Handle;
    let value = &native::read("blend_curve_symmetry_pilot")["results"][0]["value"];
    let doc = reference_inputs::setup(value, &value["prescribed_case"]);
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    let mut input = Input::new(doc.tolerance());
    for (i, p) in [[0., 0., 0.], [10., 3., 0.]].into_iter().enumerate() {
        input
            .pick_curve(
                &doc,
                reference_inputs::guide(&doc, i),
                None,
                native::point(&serde_json::json!(p)),
            )
            .unwrap();
    }
    input
        .select_handle_with_symmetry(Handle::StartShape, true)
        .unwrap();
    assert_eq!(input.paired_handle(), Some(Handle::EndShape));
    input.adjust_shape_length(3.).unwrap();
    let before = input.preview().cloned();
    let args = input.arguments().unwrap();
    for length in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(input.adjust_shape_length(length).is_err());
        assert_eq!(input.preview(), before.as_ref());
        assert_eq!(input.arguments().unwrap(), args);
        assert_eq!(input.paired_handle(), Some(Handle::EndShape));
    }
    assert!(
        input
            .adjust_point(native::point(&serde_json::json!([-1., 0., 0.])))
            .is_err()
    );
    assert_eq!(input.preview(), before.as_ref());
    input.finish_adjustment();
    assert_eq!(input.paired_handle(), None);
    input.select_handle(Handle::EndShape).unwrap();
    input.adjust_shape_length(4.).unwrap();
    input.finish_adjustment();
    let curve = input.preview().unwrap();
    let p = curve.control_points();
    assert!((p[0].point().distance_to(p[1].point()).unwrap() - 3.).abs() < 1e-12);
    assert!(
        (p[p.len() - 1]
            .point()
            .distance_to(p[p.len() - 2].point())
            .unwrap()
            - 4.)
            .abs()
            < 1e-12
    );
    input
        .select_handle_with_symmetry(Handle::Start, true)
        .unwrap();
    assert_eq!(input.paired_handle(), None);
    input
        .adjust_point(native::point(&serde_json::json!([-1., 0., 0.])))
        .unwrap();
    let curve = input.preview().unwrap();
    assert_eq!(
        curve.evaluate(*curve.domain().end()).unwrap().to_array(),
        [10., 3., 0.]
    );
    input.finish_adjustment();
    input.option("Continuity_1=G1").unwrap();
    input
        .select_handle_with_symmetry(Handle::StartShape, true)
        .unwrap();
    assert_eq!(input.paired_handle(), None);
    input.reset(doc.tolerance());
    assert_eq!(input.active_handle(), None);
    assert_eq!(input.paired_handle(), None);
    assert_eq!(input.reference_count(), 0);
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), objects);
    assert!(!doc.can_undo());
}
