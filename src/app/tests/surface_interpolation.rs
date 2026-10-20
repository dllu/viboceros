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
fn surface_interpolation_draft_undo_relative_points_and_unpicked_hover() {
    let mut app = test_app();
    let id = scene(&mut app);
    let source = app.document.object(id).unwrap().geometry_snapshot().clone();
    enter(&mut app, "InterpCrvOnSrf");
    assert!(app.picking_surface_interpolation_face());
    app.accept_component_face_hit(id, 0, None);
    enter(&mut app, "-3,-2,7");
    assert_eq!(
        app.active_command.unwrap().anchor(),
        Some(point(-3., -2., 0.))
    );
    enter(&mut app, "r3,4,0");
    enter(&mut app, "3,-1,0");
    enter(&mut app, "Undo");
    assert_eq!(
        app.surface_interpolation_session
            .as_ref()
            .unwrap()
            .points()
            .len(),
        2
    );
    assert_eq!(
        app.active_command.unwrap().anchor(),
        Some(point(0., 2., 0.))
    );
    enter(&mut app, "3,1,0");
    let expected = app
        .surface_interpolation_session
        .as_ref()
        .unwrap()
        .preview_curves()[0]
        .clone();
    let before = format!("{:?}", app.document);
    app.update_surface_interpolation_hover(point(4., -4., 0.));
    assert_eq!(before, format!("{:?}", app.document));
    enter(&mut app, "");
    assert!(
        app.surface_interpolation_session.is_none(),
        "{:?}",
        app.command_log
    );
    assert_eq!(
        app.document.objects().last().unwrap().geometry(),
        &Geometry::NurbsCurve(expected)
    );
    assert!(source.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
    assert_eq!(app.document.undo_label(), Some("InterpCrvOnSrf"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().count(), 1);
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 2);
}
#[test]
fn surface_interpolation_auto_close_waits_for_enter_and_alt_suspends_it() {
    for suspended in [false, true] {
        let mut app = test_app();
        let id = scene(&mut app);
        enter(&mut app, &format!("InterpCrvOnSrf Surface={id}"));
        for s in ["-3,-2,0", "3,-2,0", "0,3,0"] {
            enter(&mut app, s);
        }
        assert_eq!(
            app.surface_interpolation_session
                .as_ref()
                .unwrap()
                .close_point(),
            Some(point(-3., -2., 0.))
        );
        app.suspend_surface_interpolation_close(suspended);
        assert_eq!(
            app.surface_interpolation_session
                .as_ref()
                .unwrap()
                .close_point()
                .is_none(),
            suspended
        );
        app.accept_surface_interpolation_point(point(-3., -2., 0.));
        assert!(app.surface_interpolation_session.is_some());
        assert_eq!(app.document.objects().count(), 1);
        let c = &app
            .surface_interpolation_session
            .as_ref()
            .unwrap()
            .preview_curves()[0];
        assert!(c.is_closed().unwrap());
        let a = c.evaluate_with_derivative(*c.domain().start()).unwrap().1;
        let b = c.evaluate_with_derivative(*c.domain().end()).unwrap().1;
        let cosine = a
            .normalized_nonzero()
            .unwrap()
            .as_vector()
            .dot(b.normalized_nonzero().unwrap().as_vector())
            .unwrap();
        assert_eq!(cosine > 0.999999, !suspended);
        enter(&mut app, "");
        assert_eq!(app.document.objects().count(), 2);
    }
}
#[test]
fn surface_interpolation_persistent_close_cancel_redo_and_stale_target() {
    let mut app = test_app();
    let id = scene(&mut app);
    enter(
        &mut app,
        &format!("InterpCrvOnSrf Surface={id} PersistentClose=Yes"),
    );
    for s in ["-3,-2,0", "3,-2,0", "0,3,0"] {
        enter(&mut app, s);
    }
    assert!(
        app.surface_interpolation_session
            .as_ref()
            .unwrap()
            .preview_curves()[0]
            .is_closed()
            .unwrap()
    );
    enter(&mut app, "");
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, &format!("InterpCrvOnSrf Surface={id}"));
    enter(&mut app, "0,0,0");
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, &format!("InterpCrvOnSrf Surface={id}"));
    app.document.redo().unwrap();
    assert!(!app.validate_surface_interpolation());
    assert!(app.surface_interpolation_session.is_none());
}
#[path = "../../../crates/viboceros-command/tests/support/surface_interpolation.rs"]
mod native;
#[test]
fn surface_interpolation_native_getters_match_geometry_layers_selection_and_history() {
    for (op, v) in native::rows() {
        if matches!(op["case"].as_str(), Some("persistent" | "autoclose_no")) {
            println!(
                "SURFACE_INTERPOLATION_GETTER {}",
                serde_json::json!({"id":op["id"],"native_supported":false,"qualified":false})
            );
            continue;
        }
        let mut app = test_app();
        let (doc, target) = native::setup(&v);
        app.document = doc;
        let parent = app
            .document
            .object(target)
            .unwrap()
            .geometry_snapshot()
            .clone();
        enter(&mut app, "InterpCrvOnSrf");
        for action in op["actions"].as_array().unwrap() {
            if let Some(word) = action.as_str() {
                enter(&mut app, if word == "Enter" { "" } else { word });
            } else {
                let physical = action["click"].is_array();
                let a = if physical { &action["click"] } else { action };
                let p = serde_json::from_value::<[f64; 3]>(a.clone()).unwrap();
                app.update_surface_interpolation_hover(Point3::try_from(p).unwrap());
                if physical {
                    app.accept_surface_interpolation_point(Point3::try_from(p).unwrap());
                } else {
                    enter(&mut app, &p.map(|x| x.to_string()).join(","));
                }
            }
        }
        assert!(
            app.surface_interpolation_session.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        assert!(
            parent.shares_storage_with(app.document.object(target).unwrap().geometry_snapshot())
        );
        let mut maximum = native::check(&app.document, target, &v, "after_script");
        if op["case"] != "cancel" {
            app.document.undo().unwrap();
            maximum = maximum.max(native::check(&app.document, target, &v, "undo"));
            app.document.redo().unwrap();
            maximum = maximum.max(native::check(&app.document, target, &v, "redo"));
        }
        println!(
            "SURFACE_INTERPOLATION_GETTER {}",
            serde_json::json!({"id":op["id"],"native_supported":true,"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6})
        );
        assert!(maximum < 1e-6, "{} native error {maximum:e}", op["id"]);
    }
}
