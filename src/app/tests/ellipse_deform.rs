use super::*;
#[test]
fn ellipse_deform_native_frontend_point_counts_and_history() {
    super::ellipse_around::assert_native_profiles(&[
        (
            include_str!("../../../tools/rhino_oracle/fixtures/ellipse_deform_shapes.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/ellipse_deform_shapes_rhino_20261017.json"
            ),
            9,
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/ellipse_deform_modes.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/ellipse_deform_modes_rhino_20261017.json"
            ),
            20,
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/ellipse_deform_counts.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/ellipse_deform_counts_rhino_20261017.json"
            ),
            4,
        ),
    ]);
}

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn ellipse_deform_count_getter_preview_locks_preferences_and_cancel_are_edit_free() {
    use viboceros_command::ellipse_input::Slot;
    let mut app = test_app();
    for text in ["Ellipse", "Deformable", "PointCount"] {
        enter(&mut app, text);
    }
    let pending = app.ellipse_session.as_ref().unwrap().input;
    assert_eq!(pending.slot(), Slot::PointCount);
    for text in ["3", "3.5", "8mm", "NaN", "1001", "w1,2,3"] {
        enter(&mut app, text);
        assert_eq!(app.ellipse_session.as_ref().unwrap().input, pending);
        assert!(app.document.objects().next().is_none());
    }
    enter(&mut app, "8");
    assert_eq!(
        app.ellipse_session
            .as_ref()
            .unwrap()
            .input
            .options()
            .point_count,
        8
    );
    for text in ["w0,0,0", "6", "w2,0,0", "3"] {
        enter(&mut app, text);
    }
    let cursor = Point3::try_new(2., 3., 4.).unwrap();
    let preview = app
        .ellipse_session
        .as_ref()
        .unwrap()
        .input
        .preview(cursor)
        .unwrap();
    assert_eq!(preview.deformable_point_count, Some(8));
    app.handle_viewport_action(ViewportOutput {
        drafting_hover: Some(cursor),
        ..Default::default()
    });
    enter(&mut app, "Enter");
    assert!(app.ellipse_session.is_none());
    let Geometry::NurbsCurve(actual) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    let expected = preview.to_nurbs().unwrap();
    for (a, b) in actual
        .control_points()
        .iter()
        .zip(expected.control_points())
    {
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-12);
    }
    assert_eq!(actual.degree(), 3);
    assert_eq!(actual.control_points().len(), 11);
    assert_eq!(app.commands.ellipse_options().point_count, 8);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Ellipse"));
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.document.undo().unwrap().is_none());
    for text in ["Ellipse", "Deformable", "PointCount=9", "Cancel"] {
        enter(&mut app, text);
    }
    assert_eq!(app.commands.ellipse_options().point_count, 8);
    for text in ["Ellipse", "Deformable", "PointCount", "Enter"] {
        enter(&mut app, text);
    }
    assert_eq!(
        app.ellipse_session
            .as_ref()
            .unwrap()
            .input
            .options()
            .point_count,
        8
    );
    assert!(
        !app.ellipse_session
            .as_ref()
            .unwrap()
            .input
            .point_count_pending()
    );
    enter(&mut app, "Cancel");
    enter(&mut app, "Ellipse");
    assert!(!app.ellipse_session.as_ref().unwrap().input.deformable());
    assert!(app.document.objects().next().is_none());
}

#[test]
fn ellipse_deform_native_count_and_axis_cancellations_leave_no_output_or_history() {
    let q: viboceros_oracle::ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/ellipse_deform_controls.json"
    ))
    .unwrap();
    let n: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/ellipse_deform_controls_rhino_20261017.json"
    ))
    .unwrap();
    assert_eq!(q.operations.len(), 2);
    for (op, row) in q.operations.iter().zip(n["results"].as_array().unwrap()) {
        let viboceros_oracle::Operation::EllipseCommand { id, fixture } = op else {
            panic!()
        };
        let mut app = test_app();
        enter(&mut app, "Ellipse");
        super::ellipse::actions(&mut app, &fixture.actions);
        assert!(app.ellipse_session.is_none());
        assert!(app.active_command.is_none());
        super::catenary::assert_model(&app, &row["value"]["after_script"], id);
        assert!(app.document.undo().unwrap().is_none());
    }
}
