use super::*;
fn enter(app: &mut VibocerosApp, word: &str) {
    app.command_input = word.into();
    app.run_command();
}
#[test]
fn arc_deformable_native_frontend_constructions_settings_and_history() {
    super::ellipse_around::assert_native_profiles_for(
        "Arc",
        &[
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/arc_deform_midpoint_qualified.json"
                ),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_midpoint_qualified_rhino_20261017.json"
                ),
                3,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_cursor.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_cursor_rhino_20261017.json"
                ),
                12,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_tangent.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_tangent_rhino_20261017.json"
                ),
                7,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_basic.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_basic_rhino_20261017.json"
                ),
                7,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_ordered.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_ordered_rhino_20261017.json"
                ),
                10,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_constructions.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_constructions_rhino_20261017.json"
                ),
                23,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_settings.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_settings_rhino_20261017.json"
                ),
                9,
            ),
        ],
    );
}
#[test]
fn arc_deformable_preview_degree_retry_cancel_and_cross_viewport_length() {
    let mut app = test_app();
    for word in [
        "Arc Deformable",
        "Degree=2",
        "PointCount=4",
        "Degree=3",
        "PointCount",
        "Enter",
        "garbage",
        "4",
    ] {
        enter(&mut app, word);
    }
    assert_eq!(
        app.arc_deform_session.unwrap().pending(),
        Some(viboceros_command::arc_deform_input::Setting::PointCount)
    );
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "5");
    enter(&mut app, "Cancel");
    assert_eq!(app.commands.arc_deformation(), Default::default());
    assert!(app.document.undo().unwrap().is_none());
    for word in [
        "Arc Deformable Degree=2 PointCount=4 Degree=3 StartPoint",
        "w4,0,0",
        "Center",
        "w0,0,0",
    ] {
        enter(&mut app, word);
    }
    let preview = app.arc_preview().unwrap();
    let output = app.arc_deform_session.unwrap().output();
    let p = Point3::try_new(0., 4., 0.).unwrap();
    let expected = output
        .curve(
            preview
                .arc(p, app.drafting_plane.unwrap(), app.document.tolerance())
                .unwrap(),
        )
        .unwrap();
    assert!(app.accept_drafting_point(p));
    let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(curve, &expected);
    assert_eq!(curve.control_points().len(), 4);
    let saved = app.commands.arc_deformation();
    for word in ["Arc Deformable", "w0,0,0", "w4,0,0", "Length"] {
        enter(&mut app, word);
    }
    assert!(app.drafting_plane.is_none());
    app.active_viewport = (app.active_viewport + 1) % app.viewports.len();
    for word in ["w7,8,9", "w10,12,9"] {
        enter(&mut app, word);
    }
    let latest = app.document.objects().last().unwrap();
    let Geometry::NurbsCurve(curve) = latest.geometry() else {
        panic!()
    };
    assert!((curve.domain().end() - 5.).abs() < 1e-12);
    assert_eq!(app.commands.arc_deformation(), saved);
    assert!(app.arc_deform_session.is_none() && app.arc_length_session.is_none());
    // A new explicit Arc command gets ordinary exact output.
    enter(&mut app, "Arc");
    assert!(app.arc_deform_session.is_none());
    enter(&mut app, "Cancel");
}
