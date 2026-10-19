use super::remove_knot::topology;
use super::*;
use serde_json::Value;
#[path = "../../../crates/viboceros-command/tests/support/remove_control_point.rs"]
mod qualification;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn remove_control_point_native_getters_preview_metadata_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/remove_control_point_command.json"))
            .unwrap(),
    )
    .unwrap();
    let mut failures = Vec::new();
    let mut unexpected = Vec::new();
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            replay(&root, &row["value"])
        })) {
            Ok(_)
                if matches!(
                    row["value"]["recipe"]["family"].as_str(),
                    Some("polycurve" | "box")
                ) => {}
            Ok(Ok(gap)) => eprintln!(
                "REMOVE_CONTROL_POINT_UI_AUDIT {}",
                serde_json::json!({"id":name,"maximum_error":gap})
            ),
            Ok(Err(())) => failures.push(name),
            Err(_) => unexpected.push(name),
        }
    }
    assert!(
        unexpected.is_empty(),
        "Unexpected workflow failures: {unexpected:?}"
    );
    assert_eq!(
        failures,
        qualification::KNOWN_GETTER_DIFFERENCES,
        "native UI getter differences changed"
    );
}
fn replay(root: &std::path::Path, v: &Value) -> Result<f64, ()> {
    let name = v["recipe"]["id"].as_str().unwrap();
    let mut app = test_app();
    let (doc, _, _, views) = viboceros_command::open_3dm_with_views(
        root.join(format!("fixtures/remove_control_point_command/{name}.3dm"))
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
    enter(&mut app, "RemoveControlPoint");
    assert_eq!(
        app.refinement_prompt.as_ref().unwrap().command_name(),
        "RemoveControlPoint"
    );
    if app.refinement_prompt.as_ref().unwrap().selecting() {
        app.apply_selection_click(SelectionClick {
            object_id: Some(id),
            mode: SelectionMode::Replace,
        });
    }
    if matches!(v["recipe"]["family"].as_str(), Some("polycurve" | "box")) {
        assert!(app.refinement_prompt.as_ref().unwrap().selecting());
        app.cancel_current_prompt_or_selection();
        assert_eq!(app.document.object(id), Some(&before));
        assert!(!app.document.can_undo());
        eprintln!("REMOVE_CONTROL_POINT_UI_UNSUPPORTED {name}");
        return Ok(0.);
    }
    for token in v["tokens"].as_array().unwrap() {
        let t = token.as_str().unwrap();
        if t == "!" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, if t == "_Enter" { "" } else { t });
        }
        if app.refinement_prompt.is_some() {
            assert_eq!(
                app.document.object(id),
                Some(&before),
                "{name}: staged source"
            );
            assert!(!app.document.can_undo());
        }
    }
    if app.refinement_prompt.is_some() && v["success"] == false {
        app.cancel_current_prompt_or_selection();
    }
    assert!(app.refinement_prompt.is_none(), "{name}: getter ended");
    let native = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        topology::check(&app.document, &v["after"], id, name)
    }));
    let actual = app.document.object(id).unwrap().clone();
    assert_eq!(actual.attributes(), before.attributes());
    assert_eq!(actual.group_ids(), before.group_ids());
    if app.document.can_undo() {
        assert_eq!(app.document.undo_label(), Some("RemoveControlPoint"));
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
        assert!(v["undo"].is_null() || qualification::KNOWN_GETTER_DIFFERENCES.contains(&name));
    }
    match native {
        Ok(gap) if gap <= 1e-8 => Ok(gap),
        _ => Err(()),
    }
}

#[test]
fn remove_control_point_click_finishes_and_escape_retains_redo() {
    use viboceros_geometry::NurbsCurve;
    let mut app = test_app();
    let p = Point3::try_new(1., 3., 0.).unwrap();
    let c = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            p,
            Point3::try_new(3., -1., 0.).unwrap(),
            Point3::try_new(5., 0., 0.).unwrap(),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let id = app.document.add_geometry(Geometry::NurbsCurve(c)).unwrap();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.object(id).unwrap().clone();
    enter(&mut app, "RemoveControlPoint Direction=V");
    assert!(
        app.refinement_prompt
            .as_ref()
            .unwrap()
            .cursor_curve()
            .is_none()
    );
    assert!(app.refinement_prompt.as_ref().unwrap().surface().is_none());
    assert_eq!(
        app.refinement_prompt
            .as_ref()
            .unwrap()
            .markers()
            .unwrap()
            .len(),
        4
    );
    assert!(app.accept_refinement_drafting_point(p).unwrap());
    assert!(app.refinement_prompt.is_none());
    assert_eq!(app.document.undo_label(), Some("RemoveControlPoint"));
    assert_ne!(
        app.document.object(id).unwrap().geometry(),
        before.geometry()
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(id), Some(&before));
    assert!(app.document.can_redo());
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "RemoveControlPoint");
    assert_eq!(
        app.commands.remove_control_point_options().direction,
        viboceros_geometry::SurfaceKnotDirection::V
    );
    assert_eq!(
        app.commands.remove_knot_options().direction,
        viboceros_geometry::SurfaceKnotDirection::U
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.refinement_prompt.is_none());
    assert_eq!(app.document.object(id), Some(&before));
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
}
