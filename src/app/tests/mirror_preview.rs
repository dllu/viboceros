//! Pending previews do not edit geometry, selection, defaults, or command history.
use super::*;
use serde_json::{Value, json};

#[path = "../../../crates/viboceros-oracle/src/test_json.rs"]
mod test_json;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn normalized(mut objects: Vec<Value>) -> Value {
    objects.sort_by_key(|object| object.to_string());
    Value::Array(objects)
}

fn native_objects(state: &Value) -> Value {
    normalized(state.as_array().unwrap().iter().map(|object| json!({
        "type": object["type"], "bounds": object["bounds"], "selected": object["selected"]
    })).collect())
}

fn objects(document: &Document) -> Value {
    normalized(
        document
            .objects()
            .map(|object| {
                let bounds = object.geometry().bounds();
                let kind = match object.geometry() {
                    Geometry::Point(_) => "Point",
                    Geometry::Brep(_) => "Brep",
                    Geometry::Polyline(_) => "Curve",
                    _ => panic!("unexpected preview witness geometry"),
                };
                json!({"type": kind, "bounds": [bounds.min().to_array(), bounds.max().to_array()],
               "selected": document.is_selected(object.id())})
            })
            .collect(),
    )
}

#[test]
fn mirror_preview_pending_state_completion_and_cancellation_match_native() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/mirror_preview.json"
    ))
    .unwrap();
    let captures: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mirror_preview.json"
    ))
    .unwrap();
    assert_eq!(
        fixtures["operations"].as_array().unwrap().len(),
        captures["results"].as_array().unwrap().len()
    );
    for (operation, capture) in fixtures["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(captures["results"].as_array().unwrap())
    {
        assert_eq!(operation["id"], capture["id"]);
        let mut app = test_app();
        app.active_viewport = 1;
        for input in [
            "Point 3,3,0",
            "Polyline 2,1,0 4,1,0 3,-1,0 2,1,0",
            "Box 2,-5,0 4,-3,0 2",
            "SelAll",
        ] {
            enter(&mut app, input);
        }
        app.document.clear_history().unwrap();
        let original = app.document.objects().cloned().collect::<Vec<_>>();
        let three_point = operation["plane"] == "ThreePoint";
        enter(
            &mut app,
            &format!(
                "Mirror {}Copy={}",
                if three_point { "3Point " } else { "" },
                if operation["copy"] == true {
                    "Yes"
                } else {
                    "No"
                }
            ),
        );
        assert!(
            app.transform_session
                .as_ref()
                .unwrap()
                .mirror_preview(app.active_command)
                .is_none()
        );
        enter(&mut app, "w0,0,0");
        if three_point {
            assert!(
                app.transform_session
                    .as_ref()
                    .unwrap()
                    .mirror_preview(app.active_command)
                    .is_none()
            );
            enter(&mut app, "w0,0,3");
        }
        for cursor in [point(0., 5., 0.), point(1., 4., 0.), point(0., 5., 0.)] {
            let preview = app
                .transform_session
                .as_ref()
                .unwrap()
                .mirror_preview(app.active_command)
                .unwrap();
            let map = preview
                .plane
                .reflection_at(
                    app.viewports[1].construction_plane(),
                    cursor,
                    app.document.tolerance(),
                )
                .unwrap();
            assert!(app.update_mirror_preview(Some(map)));
            assert!(!app.update_mirror_preview(Some(map)));
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                original
            );
            assert!(!app.document.can_undo());
            assert_eq!(app.commands.copy_default("Mirror"), Some(true));
        }
        test_json::close(
            &objects(&app.document),
            &native_objects(&capture["value"]["pending"]),
            "pending",
            1e-10,
            1e-12,
        );
        if operation["finish"] == "Cancel" {
            app.cancel_current_prompt_or_selection();
            assert!(!app.document.can_undo());
        } else {
            enter(&mut app, "w0,5,0");
            assert_eq!(app.document.undo_label(), Some("Mirror"));
        }
        test_json::close(
            &objects(&app.document),
            &native_objects(&capture["value"]["after"]),
            "completed",
            1e-10,
            1e-12,
        );
        assert!(app.active_command.is_none());
        assert!(app.transform_session.is_none());
        assert!(!app.update_mirror_preview(None));
    }
}

#[test]
fn mirror_preview_copy_option_updates_without_changing_cached_map_or_document() {
    let mut app = test_app();
    enter(&mut app, "Point 3,3,0");
    enter(&mut app, "SelAll");
    app.document.clear_history().unwrap();
    enter(&mut app, "Mirror Copy=Yes");
    enter(&mut app, "w0,0,0");
    let preview = app
        .transform_session
        .as_ref()
        .unwrap()
        .mirror_preview(app.active_command)
        .unwrap();
    let map = preview
        .plane
        .reflection_at(
            app.viewports[1].construction_plane(),
            point(0., 5., 0.),
            app.document.tolerance(),
        )
        .unwrap();
    app.update_mirror_preview(Some(map));
    enter(&mut app, "Copy=No");
    let preview = app
        .transform_session
        .as_ref()
        .unwrap()
        .mirror_preview(app.active_command)
        .unwrap();
    assert!(!preview.copy);
    assert_eq!(preview.last_transform, Some(map));
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    app.cancel_current_prompt_or_selection();
    enter(&mut app, "Mirror");
    enter(&mut app, "w0,0,0");
    assert_eq!(
        app.transform_session
            .as_ref()
            .unwrap()
            .mirror_preview(app.active_command)
            .unwrap()
            .last_transform,
        None
    );
}
