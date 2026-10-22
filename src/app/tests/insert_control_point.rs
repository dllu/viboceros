use super::remove_knot::topology;
use super::*;
use serde_json::Value;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn insert_control_point_rejects_both_and_preserves_the_staged_edit_on_invalid_options() {
    let mut app = test_app();
    let source = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 4.0, 1.0),
            point(5.0, -1.0, 2.0),
            point(8.0, 3.0, -1.0),
            point(11.0, 1.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 4.0, 4.0],
    )
    .unwrap();
    let id = app
        .document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "SetObjectName Future");
    enter(&mut app, "Undo");
    let original = app.document.object(id).unwrap().clone();
    let options = app.commands.insert_control_point_options();

    enter(&mut app, "InsertControlPoint Direction=Both Midpoint=Yes");
    assert!(app.refinement_prompt.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.object(id), Some(&original));
    assert_eq!(app.commands.insert_control_point_options(), options);
    assert!(app.document.can_redo());
    assert!(app.command_log.back().unwrap().starts_with("Error:"));

    enter(&mut app, "InsertControlPoint Direction=U Midpoint=Yes");
    assert!(app.accept_drafting_point(source.evaluate(1.5).unwrap()));
    let preview = app
        .refinement_prompt
        .as_ref()
        .unwrap()
        .scene()
        .unwrap()
        .object(id)
        .unwrap()
        .clone();
    let Geometry::NurbsCurve(inserted) = preview.geometry() else {
        panic!("NURBS curve")
    };
    assert_eq!(inserted.control_points().len(), 6);
    for input in ["Direction=Both", "Direction=Sideways", "Midpoint=Maybe"] {
        enter(&mut app, input);
        assert!(app.command_log.back().unwrap().starts_with("Error:"));
        assert_eq!(app.document.object(id), Some(&original));
        assert_eq!(
            app.refinement_prompt
                .as_ref()
                .unwrap()
                .scene()
                .unwrap()
                .object(id),
            Some(&preview)
        );
        assert!(app.document.can_redo());
    }
    enter(&mut app, "");
    assert!(app.refinement_prompt.is_none());
    assert_eq!(app.document.object(id), Some(&preview));
    assert_eq!(app.document.undo_label(), Some("InsertControlPoint"));
    assert!(!app.document.can_redo());
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(id), Some(&original));
    assert!(!app.document.can_undo());
    enter(&mut app, "Redo");
    assert_eq!(app.document.object(id), Some(&preview));
}

#[test]
fn insert_surface_control_point_uses_the_rendered_hit_location_in_the_staged_getter() {
    let mut clicked = test_app();
    clicked.execute_command("SrfPt 0,0,0 4,0,4 4,2,4 0,2,0");
    let source = clicked.document.objects().next().unwrap().id();
    clicked
        .document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let original = clicked.document.object(source).unwrap().clone();
    enter(&mut clicked, "InsertControlPoint Direction=V");
    assert!(clicked.refinement_viewport_hit(&ViewportOutput {
        picked_point: Some(point(1.5, 1.0, 1.5)),
        ..Default::default()
    }));
    assert_eq!(clicked.document.object(source), Some(&original));
    enter(&mut clicked, "");
    let clicked_geometry = clicked.document.object(source).unwrap().geometry().clone();
    assert_eq!(clicked.document.undo_label(), Some("InsertControlPoint"));

    let mut scripted = test_app();
    scripted.execute_command("SrfPt 0,0,0 4,0,4 4,2,4 0,2,0");
    let source = scripted.document.objects().next().unwrap().id();
    scripted
        .document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    scripted.execute_command("InsertControlPoint 1.5,1,1.5 Direction=V");
    assert_eq!(
        scripted.document.object(source).unwrap().geometry(),
        &clicked_geometry
    );
    scripted.execute_command("Undo");
    scripted
        .document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    scripted.execute_command("InsertControlPoint 1.5,1,0 Direction=V");
    assert_ne!(
        scripted.document.object(source).unwrap().geometry(),
        &clicked_geometry
    );
}

#[test]
fn insert_control_point_native_getters_previews_metadata_and_history() {
    matrix("command", 82);
}
#[test]
fn insert_control_point_native_extensions_and_history() {
    matrix("extensions", 24);
}
#[test]
fn insert_control_point_native_endpoints_and_junctions() {
    matrix("endpoints", 15);
}
#[test]
fn insert_control_point_native_analytic_source_charts() {
    matrix("charts", 3);
}
fn matrix(matrix: &str, count: usize) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.join(format!("observations/insert_control_point_{matrix}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(capture["results"].as_array().unwrap().len(), count);
    let mut failures = Vec::new();
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            replay(&root, &row["value"], matrix)
        })) {
            Ok(gap) if gap <= 1e-8 => eprintln!(
                "INSERT_CONTROL_POINT_UI_AUDIT {}",
                serde_json::json!({"matrix":matrix,"id":name,"maximum_error":gap})
            ),
            _ => failures.push(name),
        }
    }
    assert!(
        failures.is_empty(),
        "Native UI differences: {matrix}/{failures:?}"
    );
}

fn replay(root: &std::path::Path, v: &Value, matrix: &str) -> f64 {
    let name = v["recipe"]["id"].as_str().unwrap();
    let mut app = test_app();
    let (doc, _, _, views) = viboceros_command::open_3dm_with_views(
        root.join(format!("fixtures/insert_control_point_{matrix}/{name}.3dm"))
            .to_str()
            .unwrap(),
    )
    .unwrap();
    app.document = doc;
    app.restore_file_viewports(views);
    let id: ObjectId = v["source_id"].as_str().unwrap().parse().unwrap();
    app.document.clear_selection();
    if v["before"][0]["selected"] == true {
        app.document
            .select_object(id, SelectionMode::Replace)
            .unwrap();
    }
    app.document.clear_history().unwrap();
    let before = app.document.object(id).unwrap().clone();
    enter(&mut app, "InsertControlPoint");
    assert_eq!(
        app.refinement_prompt.as_ref().unwrap().command_name(),
        "InsertControlPoint"
    );
    if app.refinement_prompt.as_ref().unwrap().selecting() {
        app.apply_selection_click(SelectionClick {
            object_id: Some(id),
            mode: SelectionMode::Replace,
        });
    }
    for token in v["tokens"].as_array().unwrap() {
        if app.refinement_prompt.is_none() {
            break;
        }
        let token = token.as_str().unwrap();
        if token == "!" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, if token == "_Enter" { "" } else { token });
        }
        if let Some(prompt) = &app.refinement_prompt {
            assert_eq!(
                app.document.object(id),
                Some(&before),
                "{name}: immutable source"
            );
            assert!(!app.document.can_undo());
            if let Some(scene) = prompt.scene() {
                assert_eq!(scene.object(id).unwrap().attributes(), before.attributes());
            }
        }
    }
    assert!(app.refinement_prompt.is_none(), "{name}: ended");
    let actual = app.document.object(id).unwrap().clone();
    assert_eq!(actual.attributes(), before.attributes());
    assert_eq!(actual.group_ids(), before.group_ids());
    let gap = topology::check(&app.document, &v["after"], id, name);
    if app.document.can_undo() {
        assert_eq!(app.document.undo_label(), Some("InsertControlPoint"));
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(id), Some(&before));
        if !v["undo"].is_null() {
            assert!(topology::check(&app.document, &v["undo"], id, name) <= 1e-8);
        }
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(id), Some(&actual));
    } else {
        assert_eq!(actual, before);
        assert!(!app.document.can_redo());
        assert!(v["undo"].is_null());
    }
    gap
}

#[test]
fn insert_control_point_previews_cancel_retains_redo_and_options_are_independent() {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 6,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    app.execute_command("SetObjectName Future");
    app.execute_command("Undo");
    let source = app.document.object(id).unwrap().clone();
    enter(&mut app, "InsertControlPoint Midpoint=Yes");
    assert!(
        !app.refinement_prompt
            .as_ref()
            .unwrap()
            .markers()
            .unwrap()
            .is_empty()
    );
    enter(&mut app, "w2,0,0");
    assert_eq!(app.document.object(id), Some(&source));
    assert!(app.document.can_redo());
    assert_ne!(
        app.refinement_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .object(id)
            .unwrap()
            .geometry(),
        source.geometry()
    );
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.document.object(id), Some(&source));
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    enter(&mut app, "InsertControlPoint");
    assert!(app.commands.insert_control_point_options().midpoints);
    assert!(!app.commands.insert_knot_options().midpoints);
    enter(&mut app, "w4,0,0");
    enter(&mut app, "");
    assert!(app.refinement_prompt.is_none());
    assert_eq!(app.document.undo_label(), Some("InsertControlPoint"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(id), Some(&source));
}

#[test]
fn insert_control_point_can_start_in_extend_mode_and_accept_a_drafting_pick() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/insert_control_point_extensions.json"))
            .unwrap(),
    )
    .unwrap();
    let v = &capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "surface_U_8")
        .unwrap()["value"];
    let (doc, _, _, views) = viboceros_command::open_3dm_with_views(
        root.join("fixtures/insert_control_point_extensions/surface_U_8.3dm")
            .to_str()
            .unwrap(),
    )
    .unwrap();
    let mut app = test_app();
    app.document = doc;
    app.restore_file_viewports(views);
    let id: ObjectId = v["source_id"].as_str().unwrap().parse().unwrap();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.object(id).unwrap().clone();
    enter(&mut app, "InsertControlPoint Extend");
    assert!(
        app.refinement_prompt
            .as_ref()
            .unwrap()
            .hint()
            .contains("Pick control-net end")
    );
    let point =
        Point3::try_from(serde_json::from_value::<[f64; 3]>(v["picks"][0].clone()).unwrap())
            .unwrap();
    assert_eq!(app.accept_refinement_drafting_point(point), Some(true));
    assert_eq!(app.document.object(id), Some(&before));
    enter(&mut app, "");
    let gap = topology::check(&app.document, &v["after"], id, "surface_U_8");
    assert!(gap <= 1e-8);
    assert_eq!(app.document.undo_label(), Some("InsertControlPoint"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(id), Some(&before));
    eprintln!(
        "INSERT_CONTROL_POINT_STARTUP_AUDIT {}",
        serde_json::json!({"id":"surface_U_8","maximum_error":gap})
    );
}
