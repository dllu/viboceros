use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn ellipse_axis_locks_preview_and_cancellation_do_not_edit_the_model() {
    use viboceros_command::ellipse_input::Slot;
    let mut app = test_app();
    for word in ["Ellipse", "w0,0,0", "6mm", "w4,0,0", "3mm"] {
        enter(&mut app, word);
    }
    let before = app.ellipse_session.as_ref().unwrap().input;
    assert_eq!(before.slot(), Slot::SecondAxis);
    let point = Point3::try_new(0., 2., 0.).unwrap();
    let preview = before.preview(point).unwrap();
    assert_eq!(preview.ellipse.radius_x(), 6.);
    assert_eq!(preview.ellipse.radius_y(), 3.);
    assert_eq!(app.ellipse_session.as_ref().unwrap().input, before);
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "Cancel");
    assert!(app.ellipse_session.is_none());
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.document.undo_label().is_none());
}

pub(super) fn actions(app: &mut VibocerosApp, actions: &[viboceros_oracle::EllipseCommandAction]) {
    use viboceros_oracle::EllipseCommandAction as Action;
    for action in actions {
        match action {
            Action::Pick { .. } => panic!("guide scenarios use source-pick replay"),
            Action::Point([x, y, z]) => enter(app, &format!("w{x},{y},{z}")),
            Action::Relative {
                relative: [x, y, z],
            } => enter(app, &format!("r{x},{y},{z}")),
            Action::Number(value) => enter(app, &value.to_string()),
            Action::Option(word) => enter(app, word),
        }
        assert!(
            app.command_input.is_empty(),
            "{action:?}: {:?}",
            app.command_log
        );
    }
}
#[test]
fn ellipse_frontend_native_modes_axes_foci_planes_relative_input_and_history() {
    for (q, n, count) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/ellipse_command.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/ellipse_command_rhino_20261017.json"
            ),
            16,
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/ellipse_directions.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/ellipse_directions_rhino_20261017.json"
            ),
            18,
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/ellipse_final.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/ellipse_final_rhino_20261017.json"
            ),
            11,
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/ellipse_locks.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/ellipse_locks_rhino_20261017.json"
            ),
            5,
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/ellipse_planes.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/ellipse_planes_rhino_20261017.json"
            ),
            5,
        ),
    ] {
        let q: viboceros_oracle::ProbeRequest = serde_json::from_str(q).unwrap();
        let n: serde_json::Value = serde_json::from_str(n).unwrap();
        assert_eq!(q.operations.len(), count);
        for (op, row) in q.operations.iter().zip(n["results"].as_array().unwrap()) {
            let viboceros_oracle::Operation::EllipseCommand { id, fixture: f } = op else {
                panic!()
            };
            assert_eq!(id, row["id"].as_str().unwrap());
            let mut app = test_app();
            app.last_point = f.prior_point.map(|p| Point3::try_from(p).unwrap());
            if let Some([o, x, y]) = f.construction_plane {
                app.viewports[app.active_viewport].set_construction_plane(
                    Frame3::try_from_directions(
                        Point3::try_from(o).unwrap(),
                        viboceros_geometry::Vector3::try_from(x).unwrap(),
                        viboceros_geometry::Vector3::try_from(y).unwrap(),
                        app.document.tolerance(),
                    )
                    .unwrap(),
                );
            }
            if let Some(setup) = &f.setup {
                enter(&mut app, "Ellipse");
                actions(&mut app, setup);
                assert!(app.ellipse_session.is_none(), "{id}");
                app.document.undo().unwrap();
                app.document.clear_history().unwrap();
            }
            enter(&mut app, "Ellipse");
            actions(&mut app, &f.actions);
            assert!(app.ellipse_session.is_none(), "{id}: {:?}", app.command_log);
            super::catenary::assert_model(&app, &row["value"]["after"], id);
            let options = app.commands.ellipse_options();
            assert_eq!(app.document.undo().unwrap().as_deref(), Some("Ellipse"));
            super::catenary::assert_model(&app, &row["value"]["undo"], id);
            assert!(app.document.undo().unwrap().is_none());
            assert_eq!(app.commands.ellipse_options(), options);
            assert_eq!(app.document.redo().unwrap().as_deref(), Some("Ellipse"));
            super::catenary::assert_model(&app, &row["value"]["redo"], id);
            assert_eq!(app.commands.ellipse_options(), options);
        }
    }
}

#[test]
fn ellipse_enter_accepts_the_current_locked_preview_and_filters_preserve_perpendicular_size() {
    let mut app = test_app();
    for word in ["Ellipse", "w0,0,0", "6mm", "Enter"] {
        enter(&mut app, word);
    }
    assert!(app.ellipse_session.is_some());
    assert_eq!(app.document.objects().len(), 0);
    let first = Point3::try_new(2., 0., 0.).unwrap();
    app.handle_viewport_action(ViewportOutput {
        drafting_hover: Some(first),
        ..Default::default()
    });
    enter(&mut app, "Enter");
    assert_eq!(
        app.ellipse_session.as_ref().unwrap().input.reference(),
        Some(Point3::try_new(6., 0., 0.).unwrap())
    );
    enter(&mut app, "3mm");
    let point = Point3::try_new(2., -7., 0.).unwrap();
    let preview = app
        .ellipse_session
        .as_ref()
        .unwrap()
        .input
        .preview(point)
        .unwrap();
    app.handle_viewport_action(ViewportOutput {
        drafting_hover: Some(point),
        ..Default::default()
    });
    enter(&mut app, "Enter");
    assert!(app.ellipse_session.is_none());
    assert_eq!(
        app.document.objects().next().unwrap().geometry(),
        &Geometry::Ellipse(preview.ellipse)
    );
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Ellipse"));
    assert_eq!(app.document.objects().len(), 0);
    for word in ["Ellipse", "w0,0,0", "w6,0,0", ".y", "w0,3,0", "w2,99,0"] {
        enter(&mut app, word);
        if word == ".y" || word == "w0,3,0" {
            assert!(app.ellipse_session.is_some());
            assert!(app.point_filter.is_some());
        }
    }
    assert!(app.ellipse_session.is_none());
    let Geometry::Ellipse(e) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!((e.radius_x(), e.radius_y()), (6., 3.));
}
#[test]
fn ellipse_cancel_and_command_interruption_do_not_save_pending_options_or_helpers() {
    let mut app = test_app();
    for word in [
        "Ellipse FromFoci",
        "MarkFoci=Yes",
        "w0,0,0",
        "w8,0,0",
        "w2,0,0",
    ] {
        enter(&mut app, word);
    }
    assert!(app.ellipse_session.is_some());
    assert!(app.document.undo_label().is_none());
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "Cancel");
    assert!(app.ellipse_session.is_none());
    assert!(!app.commands.ellipse_options().mark_foci);
    enter(&mut app, "Ellipse Vertical");
    enter(&mut app, "Line");
    assert!(app.ellipse_session.is_none());
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Line { .. })
    ));
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.document.undo_label().is_none());
}
