use super::boolean_intersection::{enter, pick};
use super::plane_surface_native_support as native;
use super::*;
#[path = "offset_surface/cache.rs"]
mod cache;
#[path = "../../../crates/viboceros-command/tests/support/offset_surface.rs"]
#[allow(dead_code)]
pub(super) mod inputs;
#[path = "offset_surface/mouse.rs"]
mod mouse;

#[cfg(feature = "native-smlib")]
#[test]
fn offset_srf_picks_polysurfaces_and_replays_native_result_history() {
    let mut app = test_app();
    enter(&mut app, "Box 0,0 2,2 2");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    app.document.clear_selection();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "OffsetSrf 0.25 DeleteInput=Yes");
    assert!(!app.command_line_idle());
    pick(&mut app, id);
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert!(app.document.object(id).is_some());
    assert_eq!(app.document.objects().count(), 2);
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("native offset")
    };
    assert!(brep.is_solid());
    assert!((brep.signed_volume(app.document.tolerance()).unwrap() - 15.625).abs() < 1e-8);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 2);
}

#[test]
fn offset_srf_getters_match_native_selection_geometry_identity_and_real_history() {
    let mut count = 0;
    for (id, value) in inputs::rows() {
        if inputs::diagnostic(&id, &value) {
            continue;
        }
        println!("OFFSET_SURFACE_UI_CASE {id}");
        let mut app = test_app();
        app.document = inputs::setup(&value);
        app.commands
            .remember_offset_surface_options(inputs::initial(&value))
            .unwrap();
        let state = inputs::State::capture(&app.document);
        let mut error = inputs::check(&app.document, &value["before"], false);
        enter(&mut app, "OffsetSrf");
        for (index, action) in value["prescribed_case"]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let prompt = value["input_trace"][0]["prompts"][index].as_str().unwrap();
            // Escape may leave a getter before the public prompt refreshes.
            // Its final cancellation and actual history are checked below.
            if action != "Cancel" {
                if prompt.starts_with("Select surfaces or polysurfaces") {
                    assert!(app.object_prompt.is_some(), "{id}: {prompt}");
                } else {
                    let expected = if prompt.starts_with("Offset distance")
                        || prompt.starts_with("Second distance point")
                    {
                        Some("Distance")
                    } else if prompt.starts_with("Through point") {
                        Some("ThroughPoint")
                    } else {
                        assert!(
                            prompt.starts_with("Select object to flip direction"),
                            "{id}: {prompt}"
                        );
                        None
                    };
                    assert_eq!(
                        app.offset_surface_session.as_ref().unwrap().getter(),
                        expected,
                        "{id}: {prompt}"
                    );
                }
            }
            if action == "Cancel" {
            } else if prompt.starts_with("Offset distance")
                || prompt.starts_with("Second distance point")
            {
                assert_eq!(
                    app.offset_surface_session.as_ref().unwrap().getter(),
                    Some("Distance"),
                    "{id}: {prompt}"
                );
            } else if prompt.starts_with("Through point") {
                assert_eq!(
                    app.offset_surface_session.as_ref().unwrap().getter(),
                    Some("ThroughPoint"),
                    "{id}: {prompt}"
                );
            } else if prompt.starts_with("Select object to flip direction") {
                assert_eq!(
                    app.offset_surface_session.as_ref().unwrap().getter(),
                    None,
                    "{id}: {prompt}"
                );
            } else {
                assert!(app.object_prompt.is_some(), "{id}: {prompt}");
            }
            if action == "Cancel" {
                app.cancel_current_prompt_or_selection();
            } else if action == "Enter" {
                enter(&mut app, "");
            } else if let Some(i) = action.get("curve").and_then(serde_json::Value::as_u64) {
                let object = inputs::guide(&app.document, i as usize);
                pick(&mut app, object);
            } else if action.get("click").is_some() {
                let object = inputs::guide(&app.document, 0);
                app.handle_viewport_action(crate::viewport::ViewportOutput {
                    face_click: Some((object, 0)),
                    ..Default::default()
                });
            } else if action.is_array() {
                let p = action
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_f64().unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                enter(&mut app, &format!("w{p}"));
            } else if let Some(n) = action.get("number").and_then(serde_json::Value::as_str) {
                enter(&mut app, n);
            } else if let Some(n) = action.as_f64() {
                enter(&mut app, &n.to_string());
            } else {
                enter(&mut app, action.as_str().unwrap());
            }
        }
        assert!(app.command_line_idle(), "{id}: {:?}", app.command_log);
        let mut record = state.verify(&mut app.document, &value);
        error = error.max(record["maximum_geometry_error"].as_f64().unwrap());
        record["maximum_geometry_error"] = error.into();
        record["id"] = id.into();
        println!("OFFSET_SURFACE_UI {record}");
        count += 1;
    }
    println!("OFFSET_SURFACE_UI_COUNT {count}");
}

fn ready() -> (VibocerosApp, ObjectId) {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.set_transient_selection([id]).unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "Point 90,91,92");
    enter(&mut app, "Undo");
    (app, id)
}

#[test]
fn offset_srf_preview_cache_cancel_units_and_stale_sources_preserve_history() {
    let (mut app, id) = ready();
    let original = format!("{:?}", app.document);
    enter(&mut app, "OffsetSrf");
    enter(&mut app, "1/8cm");
    assert_eq!(format!("{:?}", app.document), original);
    let s = app.offset_surface_session.as_ref().unwrap();
    assert!(s.scene().is_some());
    let candidate = s
        .candidate
        .as_ref()
        .unwrap()
        .geometries()
        .next()
        .unwrap()
        .clone();
    enter(&mut app, "Distance");
    enter(&mut app, "");
    assert_eq!(
        candidate,
        *app.offset_surface_session
            .as_ref()
            .unwrap()
            .candidate
            .as_ref()
            .unwrap()
            .geometries()
            .next()
            .unwrap()
    );
    app.cancel_current_prompt_or_selection();
    assert_eq!(format!("{:?}", app.document), original);
    assert_eq!(app.commands.offset_surface_options().distance, 1.25);
    enter(&mut app, "OffsetSrf");
    assert_eq!(
        candidate,
        *app.offset_surface_session
            .as_ref()
            .unwrap()
            .candidate
            .as_ref()
            .unwrap()
            .geometries()
            .next()
            .unwrap()
    );
    app.document
        .set_object_geometry_user_text([id], "changed", Some("yes"))
        .unwrap();
    let changed = format!("{:?}", app.document);
    app.validate_offset_surface();
    assert!(app.command_line_idle());
    assert_eq!(format!("{:?}", app.document), changed);
}

#[test]
fn offset_srf_admission_options_reuse_geometry_and_scene_changes_are_detected() {
    let (mut app, id) = ready();
    let source = app.document.object(id).unwrap().geometry().clone();
    enter(&mut app, "OffsetSrf Solid=Yes");
    let candidate = app
        .offset_surface_session
        .as_ref()
        .unwrap()
        .candidate
        .as_ref()
        .unwrap()
        .geometries()
        .next()
        .unwrap();
    let address = candidate as *const Geometry;
    let geometry = candidate.clone();
    for text in ["Corner=Sharp", "DeleteInput=Yes", "Distance", ""] {
        enter(&mut app, text);
        assert_eq!(
            address,
            app.offset_surface_session
                .as_ref()
                .unwrap()
                .candidate
                .as_ref()
                .unwrap()
                .geometries()
                .next()
                .unwrap() as *const Geometry
        );
        assert!(!app.document.can_undo());
        assert!(app.document.can_redo());
    }
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.object(id).unwrap().geometry(), &geometry);
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(id).unwrap().geometry(), &source);
    enter(&mut app, "OffsetSrf");
    app.document
        .add_geometry(Geometry::Point(Point3::try_new(6., 7., 8.).unwrap()))
        .unwrap();
    let changed = format!("{:?}", app.document);
    app.validate_offset_surface();
    assert!(app.command_line_idle());
    assert_eq!(format!("{:?}", app.document), changed);
}
