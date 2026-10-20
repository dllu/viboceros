use super::*;
#[path = "../../../crates/viboceros-command/tests/support/surface_curve_offset.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn scene(app: &mut VibocerosApp) -> (ObjectId, ObjectId) {
    let source = app
        .document
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                point(-4., 0., 0.),
                point(4., 0., 0.),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let surface = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_bilinear([
                point(-10., -10., 0.),
                point(10., -10., 0.),
                point(10., 10., 0.),
                point(-10., 10., 0.),
            ])
            .unwrap(),
        ))
        .unwrap();
    (source, surface)
}

#[test]
fn surface_offset_previews_flip_number_and_atomic_history() {
    let mut app = test_app();
    let (source, surface) = scene(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "OffsetCrvOnSrf");
    assert!(app.picking_surface_offset_face());
    app.pick_surface_offset_face(Some(surface), Some(0), Some(point(0., 3., 0.)));
    let preview = app
        .surface_offset_session
        .as_ref()
        .unwrap()
        .preview_curves()
        .as_ptr();
    assert_eq!(
        app.surface_offset_session
            .as_ref()
            .unwrap()
            .preview_curves()
            .as_ptr(),
        preview
    );
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Number");
    enter(&mut app, "3");
    enter(&mut app, "Flip");
    let curves = app
        .surface_offset_session
        .as_ref()
        .unwrap()
        .preview_curves();
    assert_eq!(curves.len(), 3);
    for (i, c) in curves.iter().enumerate() {
        let q = c.evaluate(c.parameter_at(0.5).unwrap()).unwrap();
        assert!((q.y() + (i + 1) as f64).abs() < 1e-8);
    }
    enter(&mut app, "Enter");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(app.document.undo_label(), Some("OffsetCrvOnSrf"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().count(), 2);
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 5);
    enter(&mut app, "OffsetCrvOnSrf");
    app.pick_surface_offset_face(Some(surface), None, Some(point(0., 3., 0.)));
    let session = app.surface_offset_session.as_ref().unwrap();
    assert_eq!(session.preview_curves().len(), 3);
    assert!(
        session.preview_curves()[0]
            .evaluate(session.preview_curves()[0].parameter_at(0.5).unwrap())
            .unwrap()
            .y()
            > 0.
    );
    app.cancel_current_prompt_or_selection();
}

#[test]
fn surface_offset_escape_stale_captures_and_failed_options_preserve_redo() {
    let mut app = test_app();
    let (source, surface) = scene(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Move 0,0,0 1,0,0");
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "OffsetCrvOnSrf");
    app.pick_surface_offset_face(Some(surface), None, Some(point(0., 2., 0.)));
    enter(&mut app, "Number=0");
    assert!(app.surface_offset_session.is_some());
    app.cancel_current_prompt_or_selection();
    assert_eq!(format!("{:?}", app.document), before);
    app.document.redo().unwrap();
    app.document.undo().unwrap();
    enter(&mut app, "OffsetCrvOnSrf");
    app.pick_surface_offset_face(Some(surface), None, None);
    app.document
        .replace_object_geometries([(surface, Geometry::Point(point(0., 0., 0.)))])
        .unwrap();
    assert!(!app.validate_surface_offset());
    assert!(app.active_command.is_none());
    assert!(app.surface_offset_session.is_none());
}

#[test]
fn surface_offset_temporary_curve_input_is_local_and_directional() {
    let mut app = test_app();
    let (source, surface) = scene(&mut app);
    let before = format!("{:?}", app.document);
    enter(&mut app, "OffsetCrvOnSrf");
    enter(&mut app, "SubCrv");
    app.pick_surface_offset_curve(Some(source), None);
    app.accept_surface_offset_point(point(2., 0., 0.));
    app.update_surface_offset_hover(point(-2., 0., 0.));
    assert_eq!(
        app.surface_offset_session
            .as_ref()
            .unwrap()
            .preview_curves()
            .len(),
        1
    );
    assert_eq!(format!("{:?}", app.document), before);
    app.cancel_current_prompt_or_selection();
    assert!(app.active_command.is_some());
    assert!(app.picking_surface_offset_curve());
    enter(&mut app, "SubCrv");
    app.pick_surface_offset_curve(Some(source), None);
    app.accept_surface_offset_point(point(2., 0., 0.));
    app.accept_surface_offset_point(point(-2., 0., 0.));
    assert!(app.picking_surface_offset_face());
    app.pick_surface_offset_face(Some(surface), None, Some(point(0., -3., 0.)));
    enter(&mut app, "1");
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
            .distance_to(point(10., -1., 0.))
            .unwrap()
            < 1e-8
    );
    assert!(
        c.end_point()
            .unwrap()
            .distance_to(point(-10., -1., 0.))
            .unwrap()
            < 1e-8
    );
}

#[test]
fn surface_curve_offset_native_getters_keep_selection_layers_and_grouped_history() {
    for (op, value) in native::rows() {
        let mut app = test_app();
        let (doc, scene) = native::setup(&op, &value);
        app.document = doc;
        let before = format!("{:?}", app.document);
        enter(&mut app, "OffsetCrvOnSrf");
        // The captures run in one Rhino process and retain Number. Replay each
        // scene independently with the captured final Number explicitly set.
        for action in op["actions"].as_array().unwrap() {
            if let Some(index) = action["curve"].as_u64() {
                if index == 0 {
                    assert!(app.pick_surface_offset_curve(Some(scene.source), None));
                } else {
                    assert!(app.pick_surface_offset_face(Some(scene.target), Some(0), None));
                }
            } else if let Some(text) = action.as_str() {
                enter(&mut app, text);
            } else {
                enter(&mut app, &format!("Number={}", native::number(&value)));
                assert_eq!(format!("{:?}", app.document), before);
                enter(&mut app, &action.as_f64().unwrap().to_string());
            }
        }
        assert!(
            app.active_command.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        let mut maximum = native::check(&app.document, &scene, &value, "after_script");
        app.document.undo().unwrap();
        maximum = maximum.max(native::check(&app.document, &scene, &value, "undo"));
        assert!(app.document.undo().unwrap().is_none());
        app.document.redo().unwrap();
        maximum = maximum.max(native::check(&app.document, &scene, &value, "redo"));
        native::qualify(&op, maximum);
    }
}
