use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn interpolation_plane_picker_returns_to_point_input_without_model_edits() {
    for (mode, definition) in [
        ("WorldXY", vec![]),
        ("ThroughPoint", vec!["w0,0,4"]),
        ("3Point", vec!["w0,0,4", "w10,0,4", "w0,10,4"]),
    ] {
        let mut app = test_app();
        let initial = app.viewports[0].construction_plane();
        enter(&mut app, "InterpCrvOnSrf");
        enter(&mut app, "IP");
        assert!(!app.picking_surface_interpolation_face());
        enter(&mut app, mode);
        for p in definition {
            enter(&mut app, p);
        }
        assert!(!app.picking_surface_interpolation_plane());
        assert_eq!(app.document.objects().len(), 0);
        assert!(
            app.surface_interpolation_session.is_some(),
            "{:?}",
            app.command_log
        );
        assert!(
            app.surface_interpolation_session
                .as_ref()
                .unwrap()
                .cursor_surface()
                .is_none()
        );
        for p in ["w-4,-3,5", "w-2,2,-1", "w0,-1.5,3", "w2,3,7"] {
            enter(&mut app, p);
        }
        assert_eq!(
            app.surface_interpolation_session
                .as_ref()
                .unwrap()
                .points()
                .len(),
            4,
            "{:?}",
            app.command_log
        );
        let preview = app
            .surface_interpolation_session
            .as_ref()
            .unwrap()
            .preview_curves()[0]
            .clone();
        enter(&mut app, "Undo");
        assert_eq!(
            app.surface_interpolation_session
                .as_ref()
                .unwrap()
                .points()
                .len(),
            3
        );
        enter(&mut app, "w2,3,7");
        enter(&mut app, "");
        assert!(
            app.surface_interpolation_session.is_none(),
            "{:?}",
            app.command_log
        );
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(app.viewports[0].construction_plane(), initial);
        assert_eq!(
            app.document.objects().next().unwrap().geometry(),
            &Geometry::NurbsCurve(preview)
        );
        assert_eq!(app.document.undo_label(), Some("InterpCrvOnSrf"));
        app.document.undo().unwrap();
        assert_eq!(app.document.objects().len(), 0);
    }
}

#[test]
fn interpolation_plane_abandonment_returns_to_surface_selection_and_preserves_redo() {
    for choice in ["", "Cancel"] {
        let mut app = test_app();
        app.commands
            .execute(&mut app.document, "Point 1,2,3")
            .unwrap();
        app.document.undo().unwrap();
        let before = format!("{:?}", app.document);
        enter(&mut app, "InterpCrvOnSrf");
        enter(&mut app, "IP");
        enter(&mut app, "3Point");
        enter(&mut app, "w0,0,2");
        enter(&mut app, choice);
        assert!(app.picking_surface_interpolation_face());
        assert!(app.surface_interpolation_session.is_some());
        assert!(app.intersection_prompt.is_none());
        app.cancel_current_prompt_or_selection();
        assert!(app.surface_interpolation_session.is_none());
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.document.can_redo());
    }
}

#[test]
fn interpolation_vertical_plane_finishes_after_two_points_and_freezes_its_location() {
    let mut app = test_app();
    enter(&mut app, "InterpCrvOnSrf");
    enter(&mut app, "IP");
    enter(&mut app, "Vertical");
    enter(&mut app, "w0,3,0");
    enter(&mut app, "w10,3,0");
    assert!(!app.picking_surface_interpolation_plane());
    let frame = app.viewports[0]
        .construction_plane()
        .with_origin(point(0., 99., 0.));
    app.viewports[0].set_construction_plane(frame);
    for text in ["w-4,-3,5", "w2,2,-1", "w4,-1.5,3"] {
        enter(&mut app, text);
    }
    assert!(
        app.surface_interpolation_session
            .as_ref()
            .unwrap()
            .points()
            .iter()
            .all(|p| (p.y() - 3.).abs() < 1e-12)
    );
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.viewports[0].construction_plane(), frame);
}
