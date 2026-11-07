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
mod sources;
use viboceros_command::arc_blend::{Handle, Input};
use viboceros_document::Document;
#[test]
fn circular_contact_points_use_native_parameters_instead_of_rational_parameters() {
    let mut doc = Document::default();
    let p = |x, y| viboceros_geometry::Point3::try_new(x, y, 0.).unwrap();
    let a = reference_inputs::geometry(
        &serde_json::json!({"type":"arc","points":[[-5.,-5.,0.],[-3.,-1.,0.],[0.,0.,0.]]}),
    );
    let b = reference_inputs::geometry(
        &serde_json::json!({"type":"line","start":[10.,3.,0.],"end":[15.,3.,0.]}),
    );
    let a = doc.add_geometry(a).unwrap();
    let b = doc.add_geometry(b).unwrap();
    let mut input = Input::new(doc.tolerance());
    input.pick_curve(&doc, a, None, p(0., 0.)).unwrap();
    input.pick_curve(&doc, b, None, p(10., 3.)).unwrap();
    input.select_handle(Handle::Start).unwrap();
    input.adjust_point(p(-3., -1.)).unwrap();
    let error = input.handles().unwrap()[0]
        .distance_to(p(-3., -1.))
        .unwrap();
    assert!(
        error < 1e-10,
        "native/rational parameter mismatch: {error:e}"
    );
}

#[test]
fn nonlinear_contacts_and_unit_calculations_match_native_geometry_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in sources::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if sources::diagnostic(name) {
                continue;
            }
            let value = &row["value"];
            let id = format!("{family}/{name}");
            assert_eq!(
                value["input_trace"][0]["errors"],
                serde_json::json!([]),
                "{id}"
            );
            let mut doc = sources::setup(value);
            let mut input = Input::new(doc.tolerance());
            sources::replay(&mut input, &doc, value);
            let target = input.radius_difference();
            sources::check_target(target, sources::target(value));
            let warning = input.warning().is_some();
            assert_eq!(
                warning,
                value["history"]
                    .as_str()
                    .unwrap()
                    .contains("Unable to achieve")
            );
            let registry = viboceros_command::CommandRegistry::with_builtins();
            registry
                .execute(&mut doc, &sources::command(&input, name, value))
                .unwrap();
            if target.is_some() {
                sources::check_target(
                    Some(registry.arc_blend_preferences().radius_difference),
                    target,
                );
            }
            let (samples, stations) = sources::samples(&doc, value);
            let mut error = inputs::check(&doc, &value["after"], false);
            doc.undo().unwrap();
            error = error.max(inputs::check(&doc, &value["undo"], false));
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            error = error.max(inputs::check(&doc, &value["redo"], false));
            let path = directory.path().join("sources.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            registry
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            assert_eq!(restored.units(), doc.units());
            error = error.max(inputs::check(&restored, &value["after"], true));
            println!(
                "ARC_BLEND_SOURCE_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_arc_sample_error":samples,"native_stations":stations,"archives":1,"requested_radius_difference":target,"warning":warning})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 35);
}

#[test]
fn whole_expression_suffixes_in_scripted_coordinates_match_native_point_getters() {
    let directory = tempfile::tempdir().unwrap();
    for row in native::read("arc_blend_point_quantities")["results"]
        .as_array()
        .unwrap()
    {
        let value = &row["value"];
        let id = row["id"].as_str().unwrap();
        let mut doc = sources::setup(value);
        let words = value["prescribed_case"]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["number"].as_str().unwrap())
            .collect::<Vec<_>>();
        let registry = viboceros_command::CommandRegistry::with_builtins();
        registry
            .execute(&mut doc, &format!("Line {}", words.join(" ")))
            .unwrap();
        let mut error = inputs::check(&doc, &value["after"], false);
        doc.undo().unwrap();
        error = error.max(inputs::check(&doc, &value["undo"], false));
        doc.redo().unwrap();
        error = error.max(inputs::check(&doc, &value["redo"], false));
        let path = directory.path().join("point-units.3dm");
        registry
            .execute(&mut doc, &format!("SaveAs {}", path.display()))
            .unwrap();
        let mut restored = Document::new(doc.tolerance());
        registry
            .execute(&mut restored, &format!("Open3dm {}", path.display()))
            .unwrap();
        error = error.max(inputs::check(&restored, &value["after"], true));
        println!(
            "ARC_BLEND_SOURCE_POINT_COMMAND {}",
            serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1})
        );
        assert!(error <= 1e-6);
    }
}
