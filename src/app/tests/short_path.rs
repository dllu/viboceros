use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn scene(app: &mut VibocerosApp) -> ObjectId {
    app.document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_bilinear([
                point(-5., -5., 0.),
                point(5., -5., 0.),
                point(5., 5., 0.),
                point(-5., 5., 0.),
            ])
            .unwrap(),
        ))
        .unwrap()
}
#[test]
fn short_path_getters_preview_commit_and_undo_without_mutating_source() {
    let mut app = test_app();
    let id = scene(&mut app);
    let source = app.document.object(id).unwrap().geometry_snapshot().clone();
    enter(&mut app, "ShortPath");
    assert!(app.picking_short_path_face());
    app.accept_component_face_hit(id, 0, None);
    assert!(!app.picking_short_path_face());
    enter(&mut app, "-4,-3,0");
    let before = format!("{:?}", app.document);
    app.update_short_path_hover(point(4., 3., 0.));
    let preview = app.short_path_session.as_ref().unwrap().preview_curves();
    assert_eq!(preview.len(), 1, "{:?}", app.command_log);
    let expected = preview[0].clone();
    let pointer = preview.as_ptr();
    app.update_short_path_hover(point(4., 3., 0.));
    assert_eq!(
        pointer,
        app.short_path_session
            .as_ref()
            .unwrap()
            .preview_curves()
            .as_ptr()
    );
    assert_eq!(before, format!("{:?}", app.document));
    enter(&mut app, "4,3,0");
    assert!(app.short_path_session.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().count(), 2);
    assert!(source.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
    assert_eq!(
        app.document.objects().last().unwrap().geometry(),
        &Geometry::NurbsCurve(expected)
    );
    assert_eq!(app.document.undo_label(), Some("ShortPath"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().count(), 1);
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 2);
}
#[test]
fn short_path_cancel_and_stale_history_preserve_redo() {
    let mut app = test_app();
    let id = scene(&mut app);
    enter(&mut app, &format!("ShortPath 0,0,0 1,1,0 Surface={id}"));
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, &format!("ShortPath Surface={id}"));
    enter(&mut app, "0,0,0");
    app.update_short_path_hover(point(2., 2., 0.));
    app.cancel_interactive_command(false);
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, &format!("ShortPath Surface={id}"));
    enter(&mut app, "0,0,0");
    app.document.redo().unwrap();
    assert!(!app.validate_short_path());
    assert!(app.short_path_session.is_none());
    assert_eq!(app.document.objects().count(), 2);
}
#[test]
fn short_path_preselection_and_typed_surface_id_share_the_getter() {
    for preselect in [false, true] {
        let mut app = test_app();
        let id = scene(&mut app);
        if preselect {
            app.document
                .select_object(id, SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "ShortPath");
        if !preselect {
            enter(&mut app, &id.to_string());
        }
        enter(&mut app, "0,0,3");
        enter(&mut app, "1,2,5");
        assert_eq!(app.document.objects().count(), 2, "{:?}", app.command_log);
        let c = app
            .document
            .objects()
            .last()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert!(
            c.start_point()
                .unwrap()
                .distance_to(point(0., 0., 0.))
                .unwrap()
                < 1e-8
        );
        assert!(
            c.end_point()
                .unwrap()
                .distance_to(point(1., 2., 0.))
                .unwrap()
                < 1e-8
        );
    }
}

#[test]
fn short_path_relative_point_input_uses_the_projected_start() {
    let mut app = test_app();
    let id = scene(&mut app);
    enter(&mut app, &format!("ShortPath Surface={id}"));
    enter(&mut app, "1,1,7");
    assert_eq!(
        app.active_command.unwrap().anchor(),
        Some(point(1., 1., 0.))
    );
    enter(&mut app, "r1,2,0");
    assert_eq!(app.document.objects().count(), 2, "{:?}", app.command_log);
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
            .distance_to(point(2., 3., 0.))
            .unwrap()
            < 1e-8
    );
}

#[path = "../../../crates/viboceros-command/tests/support/short_path.rs"]
mod native;
#[test]
fn short_path_native_getters_keep_layers_selection_and_history() {
    for (op, v) in native::rows() {
        let mut app = test_app();
        let (doc, target) = native::setup(&v);
        app.document = doc;
        let parent = app
            .document
            .object(target)
            .unwrap()
            .geometry_snapshot()
            .clone();
        enter(&mut app, "ShortPath");
        for action in op["actions"].as_array().unwrap() {
            if let Some(word) = action.as_str() {
                enter(&mut app, word);
            } else {
                let point = serde_json::from_value::<[f64; 3]>(action.clone()).unwrap();
                app.update_short_path_hover(Point3::try_from(point).unwrap());
                enter(&mut app, &point.map(|x| x.to_string()).join(","));
            }
        }
        assert!(app.short_path_session.is_none(), "{:?}", app.command_log);
        let mut maximum = native::check(&app.document, target, &v, "after_script");
        assert!(
            parent.shares_storage_with(app.document.object(target).unwrap().geometry_snapshot())
        );
        if op["case"] != "cancel" {
            app.document.undo().unwrap();
            maximum = maximum.max(native::check(&app.document, target, &v, "undo"));
            app.document.redo().unwrap();
            maximum = maximum.max(native::check(&app.document, target, &v, "redo"));
        }
        println!(
            "SHORT_PATH_GETTER {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6})
        );
        assert_eq!(
            maximum < 1e-6,
            matches!(op["case"].as_str().unwrap(), "plane" | "cancel")
        );
        assert!(maximum < 0.001);
    }
}
