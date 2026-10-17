use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn circle_deform_native_frontend_construction_and_history() {
    super::ellipse_around::assert_native_profiles_for(
        "Circle",
        &[
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_degrees.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_degrees_rhino_20261017.json"
                ),
                10,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_default.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_default_rhino_20261017.json"
                ),
                1,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_modes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_modes_rhino_20261017.json"
                ),
                22,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_extended.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_extended_rhino_20261017.json"
                ),
                7,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_boundaries.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_boundaries_rhino_20261017.json"
                ),
                17,
            ),
        ],
    );
}

#[test]
fn circle_deform_preview_captured_plane_errors_and_preferences_survive_history() {
    let mut app = test_app();
    let plane = Frame3::try_from_directions(
        Point3::try_new(1e9, -2e9, 3e9).unwrap(),
        viboceros_geometry::Vector3::try_new(0.6, 0.8, 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        app.document.tolerance(),
    )
    .unwrap();
    app.viewports[app.active_viewport].set_construction_plane(plane);
    for word in [
        "Circle Deformable",
        "Degree",
        "4",
        "PointCount",
        "12",
        "w1000000000,-2000000000,3000000000",
        "Radius",
    ] {
        enter(&mut app, word);
    }
    let cursor = plane.point_at([3., 4., 2.]).unwrap();
    let before = app.circle_deform_session.as_ref().unwrap().input;
    let preview = before.preview(cursor).unwrap();
    assert_eq!(app.circle_deform_session.as_ref().unwrap().input, before);
    assert!(app.document.undo_label().is_none());
    assert_eq!(app.document.objects().len(), 0);
    for bad in ["0", "-1", "NaN"] {
        enter(&mut app, bad);
        assert_eq!(app.circle_deform_session.as_ref().unwrap().input, before);
        assert_eq!(app.document.objects().len(), 0);
    }
    app.viewports[app.active_viewport]
        .set_construction_plane(viboceros_command::CommandContext::default().construction_plane);
    let [x, y, z] = cursor.to_array();
    enter(&mut app, &format!("w{x},{y},{z}"));
    assert!(app.circle_deform_session.is_none(), "{:?}", app.command_log);
    let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(curve, &preview);
    let prefs = app.commands.circle_preferences();
    assert_eq!(prefs.deformation.degree, 4);
    assert_eq!(prefs.deformation.point_count, 12);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Circle"));
    assert_eq!(app.commands.circle_preferences(), prefs);
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("Circle"));
    enter(&mut app, "Circle Deformable");
    enter(&mut app, "PointCount=3");
    assert_eq!(
        app.circle_deform_session
            .as_ref()
            .unwrap()
            .input
            .output()
            .deformation,
        prefs.deformation
    );
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.commands.circle_preferences(), prefs);
}

#[test]
fn circle_deform_fit_points_keeps_output_options_during_postselection() {
    let mut app = test_app();
    let ids = [[3., 0., 0.], [0., 3., 0.], [-3., 0., 0.]]
        .into_iter()
        .map(|p| {
            app.document
                .add_geometry(Geometry::Point(Point3::try_from(p).unwrap()))
                .unwrap()
        })
        .collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    for word in [
        "Circle Deformable",
        "Degree=4",
        "PointCount=12",
        "FitPoints",
    ] {
        enter(&mut app, word);
    }
    assert!(app.object_prompt.is_some());
    for id in &ids {
        app.handle_viewport_action(ViewportOutput {
            selection_click: Some(crate::viewport::SelectionClick {
                object_id: Some(*id),
                mode: SelectionMode::Add,
            }),
            ..Default::default()
        });
    }
    enter(&mut app, "");
    assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
    let Geometry::NurbsCurve(curve) = app
        .document
        .objects()
        .find(|o| !ids.contains(&o.id()))
        .unwrap()
        .geometry()
    else {
        panic!("{:?}", app.command_log)
    };
    assert_eq!(curve.degree(), 4);
    assert_eq!(curve.control_points().len(), 16);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Circle"));
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("Circle"));
    assert_eq!(app.document.objects().len(), 4);
}

#[test]
fn circle_deform_point_getters_use_common_filters_and_distance_constraints() {
    let mut app = test_app();
    for word in ["Circle Deformable", "2Point", "w0,0,0", "6"] {
        enter(&mut app, word);
    }
    assert!(app.point_constraint.is_some());
    enter(&mut app, "w3,4,0");
    assert!(app.circle_deform_session.is_none(), "{:?}", app.command_log);
    assert!((app.commands.circle_preferences().radius - 1.).abs() < 1e-12);
    app.document.undo().unwrap();
    for word in [
        "Circle Deformable",
        ".xy",
        "w1,2,99",
        "w99,99,3",
        "Area",
        "9mm",
    ] {
        enter(&mut app, word);
    }
    assert_eq!(app.document.objects().len(), 0);
    assert_eq!(
        app.circle_deform_session
            .as_ref()
            .unwrap()
            .input
            .getter()
            .anchor()
            .unwrap()
            .to_array(),
        [1., 2., 3.]
    );
    enter(&mut app, "9*pi");
    assert!(app.circle_deform_session.is_none(), "{:?}", app.command_log);
    assert!((app.commands.circle_preferences().radius - 3.).abs() < 1e-12);
}

#[test]
fn circle_deform_native_cancelled_getters_leave_no_model_history() {
    let q: viboceros_oracle::ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/circle_deform_controls.json"
    ))
    .unwrap();
    let n: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/circle_deform_controls_rhino_20261017.json"
    ))
    .unwrap();
    assert_eq!(q.operations.len(), 4);
    for (op, row) in q.operations.iter().zip(n["results"].as_array().unwrap()) {
        let viboceros_oracle::Operation::CircleDeform { id, fixture } = op else {
            panic!()
        };
        let mut app = test_app();
        enter(&mut app, "Circle");
        super::ellipse::actions(&mut app, &fixture.actions);
        assert!(app.circle_deform_session.is_none());
        assert!(app.active_command.is_none());
        super::catenary::assert_model_excluding(&app, &row["value"]["after_script"], id, &[]);
        assert!(app.document.undo().unwrap().is_none());
        assert!(app.document.redo().unwrap().is_none());
    }
}

#[test]
fn circle_deform_unsupported_layout_can_be_corrected_before_any_point_is_accepted() {
    let mut app = test_app();
    for word in ["Circle Deformable", "Degree=11", "PointCount=13", "w0,0,0"] {
        enter(&mut app, word);
    }
    assert!(app.circle_deform_session.as_ref().unwrap().input.at_start());
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.document.undo_label().is_none());
    for word in ["PointCount=16", "w0,0,0", "3"] {
        enter(&mut app, word);
    }
    assert!(app.circle_deform_session.is_none(), "{:?}", app.command_log);
    let Geometry::NurbsCurve(c) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(c.degree(), 11);
    assert_eq!(c.control_points().len(), 27);
}
