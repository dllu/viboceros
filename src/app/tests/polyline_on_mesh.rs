use super::*;
#[path = "../../../crates/viboceros-command/tests/support/mesh_polyline.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn app_scene() -> (VibocerosApp, ObjectId) {
    let (_, v) = native::rows().remove(0);
    let (doc, id) = native::setup(&v);
    let mut app = test_app();
    app.document = doc;
    app.viewports[0] = Viewport::new(crate::viewport::ViewKind::Top);
    (app, id)
}
#[test]
fn mesh_polyline_native_getters_keep_mesh_sections_metadata_and_history() {
    replay(native::rows(), "MESH_POLYLINE_GETTER");
}
#[test]
fn mesh_polyline_boundary_getters_replay_native_routes_and_history() {
    replay(native::boundary_rows(), "MESH_BOUNDARY_GETTER");
}
#[test]
fn mesh_polyline_boundary_tie_getters_match_native_choices() {
    replay(native::tie_rows(), "MESH_BOUNDARY_TIE_GETTER");
}
fn replay(rows: Vec<(serde_json::Value, serde_json::Value)>, prefix: &str) {
    for (op, v) in rows {
        let (doc, id) = native::setup(&v);
        let mut app = test_app();
        app.document = doc;
        app.viewports[0] = Viewport::new(crate::viewport::ViewKind::Top);
        let parent = app.document.object(id).unwrap().geometry_snapshot().clone();
        enter(&mut app, "PolylineOnMesh");
        for action in op["actions"].as_array().unwrap() {
            if let Some(word) = action.as_str() {
                enter(&mut app, if word == "Enter" { "" } else { word });
            } else {
                let p = native::point(action);
                if let Some(index) = app
                    .mesh_polyline_session
                    .as_ref()
                    .and_then(crate::app::polyline_on_mesh::Session::index)
                    && let Some(p) = app.viewports[0].mesh_constraint_location(
                        p,
                        index,
                        app.document.tolerance(),
                    )
                {
                    app.update_mesh_polyline_hover(p);
                }
                enter(&mut app, &p.to_array().map(|v| v.to_string()).join(","));
            }
        }
        if matches!(op["id"].as_str(), Some("off_mesh")) {
            assert_eq!(app.document.objects().count(), 1);
            assert!(app.document.undo_label().is_none());
            println!(
                "{prefix} {}",
                serde_json::json!({"id":op["id"],"qualified":false,"reason":"native degenerate miss output is not reproduced"})
            );
            continue;
        }
        assert!(
            app.mesh_polyline_session.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        assert!(parent.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
        let mut maximum = native::check(&app.document, id, &v, "after_script");
        if op["id"] != "cancel" {
            assert_eq!(app.document.undo_label(), Some("PolylineOnMesh"));
            app.document.undo().unwrap();
            maximum = maximum.max(native::check(&app.document, id, &v, "undo"));
            app.document.redo().unwrap();
            maximum = maximum.max(native::check(&app.document, id, &v, "redo"));
        }
        println!(
            "{prefix} {}",
            serde_json::json!({"id":op["id"],"qualified":maximum<1e-6,"maximum_bidirectional_sample_error":maximum})
        );
        assert!(maximum < 1e-6, "{} {maximum:e}", op["id"]);
    }
}
#[test]
fn mesh_polyline_postselection_relative_input_point_undo_and_cached_enter() {
    let (mut app, id) = app_scene();
    app.document.clear_selection();
    enter(&mut app, "PolylineOnMesh");
    assert!(app.picking_mesh_polyline());
    app.accept_component_face_hit(id, 0, None);
    assert!(!app.picking_mesh_polyline());
    enter(&mut app, "-3,-2,9");
    assert_eq!(
        app.active_command.unwrap().anchor(),
        Some(point(-3., -2., 0.))
    );
    enter(&mut app, "r3,3,0");
    enter(&mut app, "2,-1,0");
    enter(&mut app, "Undo");
    assert_eq!(
        app.active_command.unwrap().anchor(),
        Some(point(0., 1., 0.))
    );
    let before = format!("{:?}", app.document);
    let index = app.mesh_polyline_session.as_ref().unwrap().index().unwrap() as *const _;
    app.update_mesh_polyline_hover(point(3., 2., 0.));
    let preview = app.mesh_polyline_session.as_ref().unwrap().preview_curves()[0].clone();
    let ptr = app
        .mesh_polyline_session
        .as_ref()
        .unwrap()
        .preview_curves()
        .as_ptr();
    app.update_mesh_polyline_hover(point(3., 2., 0.));
    assert_eq!(
        ptr,
        app.mesh_polyline_session
            .as_ref()
            .unwrap()
            .preview_curves()
            .as_ptr()
    );
    assert_eq!(
        index,
        app.mesh_polyline_session.as_ref().unwrap().index().unwrap() as *const _
    );
    assert_eq!(before, format!("{:?}", app.document));
    // Enter finishes accepted points, even while another segment is under the cursor.
    enter(&mut app, "");
    assert_eq!(app.document.objects().count(), 2);
    let c = app
        .document
        .objects()
        .last()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert!(
        c.end_point()
            .unwrap()
            .distance_to(point(0., 1., 0.))
            .unwrap()
            < 1e-8
    );
    assert_ne!(c.to_nurbs().unwrap(), preview);
}
#[test]
fn mesh_polyline_source_picks_cancellation_stale_history_and_return_close() {
    let (mut app, id) = app_scene();
    enter(&mut app, "PolylineOnMesh -3,-2,0 2,-1,0");
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, &format!("PolylineOnMesh Mesh={id}"));
    enter(&mut app, "0,0,0");
    enter(&mut app, "1,1,0");
    app.cancel_interactive_command(false);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, &format!("PolylineOnMesh Mesh={id}"));
    app.document.redo().unwrap();
    assert!(!app.validate_mesh_polyline());
    assert!(app.mesh_polyline_session.is_none());
    app.document.undo().unwrap();
    enter(&mut app, &format!("PolylineOnMesh Mesh={id}"));
    for p in ["-3,-2,0", "2,-1,0", "0,3,0", "-3,-2,0"] {
        enter(&mut app, p);
    }
    assert!(app.mesh_polyline_session.is_none());
    let Geometry::Polyline(c) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert!(c.is_closed());
}
