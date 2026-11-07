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
#[path = "support/blend_curve_edges.rs"]
mod edges;

#[path = "support/blend_curve_points.rs"]
#[allow(dead_code)]
mod points;
#[path = "support/blend_curve_portions.rs"]
#[allow(dead_code)]
mod portions;
#[test]
fn surface_edge_blends_match_native_geometry_sources_and_history() {
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
        error.max(edges::parent_edges(doc, target))
    };
    let mut count = 0;
    for family in edges::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if edges::diagnostic(name) {
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
            let mut doc = edges::setup(value);
            doc.clear_selection();
            let mut input = Input::with_preferences(doc.tolerance(), edges::preferences(value));
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
                "BLEND_CURVE_EDGES_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_sample_error":samples,"maximum_parameter_domain_error":domain_error.get(),"maximum_parameter_domain_gate_ratio":domain_ratio.get(),"maximum_tight_length_error":tight_length_error.get(),"maximum_normalized_derivative_error":derivative_error,"maximum_absolute_derivative_error":before_jets.0.max(after_jets.0),"derivative_stations":before_jets.2+after_jets.2,"native_stations":stations,"archives":1,"accepted":accepted})
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
fn unqualified_surface_orders_and_stale_parents_fail_without_mutating_the_candidate() {
    let rows = native::read("blend_curve_edge_discovery");
    let value = &rows["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "surface_curved_g2")
        .unwrap()["value"];
    let mut doc = edges::setup(value);
    let mut input = Input::with_preferences(doc.tolerance(), edges::preferences(value));
    blend::replay(&mut input, &doc, value);
    let before = input.preview().cloned();
    let args = input.arguments().unwrap();
    for option in [
        "Continuity_1=G3",
        "Continuity_1=G4",
        "AtAngle_1=NaN",
        "Flip1=Yes",
    ] {
        assert!(input.option(option).is_err(), "{option}");
        assert_eq!(input.preview(), before.as_ref());
        assert_eq!(input.arguments().unwrap(), args);
    }
    let parent = reference_inputs::guide(&doc, 0);
    let geometry = doc
        .object(parent)
        .unwrap()
        .geometry()
        .transformed(
            viboceros_geometry::AffineTransform3::from_translation(
                viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap(),
            ),
            doc.tolerance(),
        )
        .unwrap();
    doc.replace_object_geometries([(parent, geometry)]).unwrap();
    doc.clear_undo_records().unwrap();
    assert!(!input.current(&doc));
    assert!(input.commit(&mut doc).is_err());
    assert_eq!(doc.objects().len(), 3);
    assert!(!doc.can_undo());
}
