use super::*;
use viboceros_geometry::Vector3;
#[path = "../../../crates/viboceros-command/tests/support/interpolation_plane.rs"]
#[allow(dead_code)]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn plane_interpolation_native_workflows_use_nested_getters_history_and_archives() {
    let captured = native::read("surface_interpolate_plane_workflows");
    let audit = native::read("surface_interpolate_plane_audit");
    assert_eq!(captured["results"].as_array().unwrap().len(), 20);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("plane-interpolation.3dm");
    assert_eq!(audit["results"].as_array().unwrap().len(), 3);
    for row in captured["results"]
        .as_array()
        .unwrap()
        .iter()
        .chain(audit["results"].as_array().unwrap())
    {
        let v = &row["value"];
        let (doc, source) = native::setup(v);
        let parents = doc.objects().cloned().collect::<Vec<_>>();
        let mut app = test_app();
        app.document = doc;
        let initial = app.viewports[0].construction_plane();
        let id = row["id"].as_str().unwrap();
        let plane = v["plane"].as_array().unwrap();
        let frame = Frame3::try_from_unit_normal(
            Point3::try_new(
                plane[0].as_f64().unwrap(),
                plane[1].as_f64().unwrap(),
                plane[2].as_f64().unwrap(),
            )
            .unwrap(),
            Vector3::try_new(
                plane[3].as_f64().unwrap(),
                plane[4].as_f64().unwrap(),
                plane[5].as_f64().unwrap(),
            )
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        )
        .unwrap();
        if id.ends_with("_named") {
            app.viewports[0].set_construction_plane(frame);
            enter(&mut app, "NamedCPlane Save InterpolationPlane");
            app.viewports[0].set_construction_plane(initial);
        }
        if id.ends_with("_cplane") {
            app.viewports[0].set_construction_plane(frame);
        }
        let expected_cplane = app.viewports[0].construction_plane();
        enter(&mut app, "InterpCrvOnSrf");
        for word in v["picker"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            let word = if word == "@reference" {
                app.document
                    .objects()
                    .find(|o| o.attributes().name() == Some("plane-reference"))
                    .unwrap()
                    .id()
                    .to_string()
            } else {
                word.into()
            };
            enter(&mut app, &word);
        }
        if !v["control"].as_bool().unwrap() {
            assert!(
                app.surface_interpolation_session.is_some(),
                "{id}: {:?}",
                app.command_log
            );
            assert!(
                !app.picking_surface_interpolation_plane(),
                "{id}: {:?}",
                app.command_log
            );
            for action in v["input_trace"][0]["sent"].as_array().unwrap() {
                let token = action["token"].as_str().unwrap();
                enter(&mut app, if token == "_Enter" { "" } else { token });
            }
        }
        assert!(
            app.surface_interpolation_session.is_none(),
            "{id}: {:?}",
            app.command_log
        );
        assert!(app.intersection_prompt.is_none());
        assert_eq!(app.viewports[0].construction_plane(), expected_cplane);
        let mut error = native::check(&app.document, source, v, "after");
        if v["success"] == true {
            assert_eq!(app.document.undo_label(), Some("InterpCrvOnSrf"));
            app.document.undo().unwrap();
            error = error.max(native::check(&app.document, source, v, "undo"));
            app.document.redo().unwrap();
            error = error.max(native::check(&app.document, source, v, "redo"));
        } else {
            assert!(!app.document.can_undo());
        }
        for parent in parents {
            let current = app.document.object(parent.id()).unwrap();
            assert_eq!(current, &parent);
            assert!(
                current
                    .geometry_snapshot()
                    .shares_storage_with(parent.geometry_snapshot())
            );
        }
        app.commands
            .execute(&mut app.document, &format!("SaveAs {}", path.display()))
            .unwrap();
        let mut restored = Document::new(app.document.tolerance());
        app.commands
            .execute(&mut restored, &format!("Open3dm {}", path.display()))
            .unwrap();
        error = error.max(native::check(&restored, source, v, "after"));
        println!(
            "INTERPOLATION_PLANE_GETTER {}",
            serde_json::json!({"id":id,"maximum_bidirectional_sample_error":error,"geometry_qualified":error<=1e-6,"archive_objects":restored.objects().len()})
        );
        assert!(error <= 1e-6, "{id}: {error:e}");
    }
}
