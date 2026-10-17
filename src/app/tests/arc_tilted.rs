use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn tilted_arc_native_frontend_spatial_getters_and_history() {
    super::ellipse_around::assert_native_profiles_for(
        "Arc",
        &[
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tilted_initial.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tilted_initial_rhino_20261017.json"
                ),
                2,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tilted_modes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tilted_modes_rhino_20261017.json"
                ),
                11,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tilted_endings.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tilted_endings_rhino_20261017.json"
                ),
                2,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tilted_spatial.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tilted_spatial_rhino_20261017.json"
                ),
                6,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tilted_cursor.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tilted_cursor_rhino_20261017.json"
                ),
                12,
            ),
        ],
    );
}
#[test]
fn tilted_arc_orientation_can_use_another_viewport_and_preview_matches_commit() {
    let mut app = test_app();
    for word in ["Arc", "w0,0,0", "Tilted", "w4,0,0"] {
        enter(&mut app, word);
    }
    assert!(app.drafting_plane.is_none());
    let seed = app.arc_tilted_session.as_ref().unwrap().input;
    assert!(seed.requests_orientation());
    assert!(seed.orientation_guide(p(0., 3., 3.)).is_some());
    assert!(!app.accept_drafting_point(p(7., 0., 0.)));
    assert_eq!(app.arc_tilted_session.as_ref().unwrap().input, seed);
    app.active_viewport = (app.active_viewport + 1) % app.viewports.len();
    assert!(app.accept_drafting_point(p(0., 3., 3.)));
    let input = app.arc_tilted_session.as_ref().unwrap().input;
    assert_eq!(app.drafting_plane, input.frame());
    let preview = input.preview(p(0., 2., 2.)).unwrap();
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.document.undo_label().is_none());
    assert!(!app.accept_drafting_point(p(0., 0., 0.)));
    assert!(app.accept_drafting_point(p(0., 2., 2.)));
    let Geometry::Arc(arc) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(*arc, preview);
    assert_eq!(app.commands.arc_tangent_radius(), 1.);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Arc"));
    assert!(app.document.undo().unwrap().is_none());
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("Arc"));
    assert!(app.arc_tilted_session.is_none());
}
#[test]
fn tilted_arc_partial_prefix_units_constraints_and_interruption_have_one_owner() {
    let mut app = test_app();
    enter(&mut app, "A Center w0,0,0 Tilted");
    enter(&mut app, "w4,0,0");
    enter(&mut app, "w0,3,3");
    let seed = app.arc_tilted_session.as_ref().unwrap().input;
    enter(&mut app, "0");
    assert_eq!(app.arc_tilted_session.as_ref().unwrap().input, seed);
    enter(&mut app, "Length");
    enter(&mut app, "5mm");
    assert_eq!(app.document.objects().len(), 1);
    let Geometry::Arc(arc) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!((arc.radius() * arc.sweep_radians() - 5.).abs() < 1e-12);
    assert!(app.command_input.is_empty());
    for words in [
        vec!["Arc", "w0,0,0", "Tilted"],
        vec!["Arc", "w0,0,0", "Tilted", "w4,0,0"],
        vec!["Arc", "w0,0,0", "Tilted", "w4,0,0", "w0,3,3"],
    ] {
        for word in words {
            enter(&mut app, word);
        }
        enter(&mut app, "Cancel");
        assert!(app.arc_tilted_session.is_none());
        assert_eq!(app.document.objects().len(), 1);
    }
    enter(&mut app, "Arc Center w0,0,0 Tilted");
    enter(&mut app, "Line");
    assert!(app.arc_tilted_session.is_none());
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Line { .. })
    ));
}

#[test]
fn tilted_arc_partial_prefix_establishes_relative_base_and_uses_common_point_filters() {
    let mut app = test_app();
    app.last_point = Some(p(30., 40., 50.));
    for word in [
        "Arc Center w2,3,4 Tilted",
        "r4,0,0",
        ".xy",
        "w2,6,100",
        "w0,0,7",
        "-90",
    ] {
        enter(&mut app, word);
    }
    assert!(app.arc_tilted_session.is_none(), "{:?}", app.command_log);
    let Geometry::Arc(arc) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(arc.start().unwrap().distance_to(p(6., 3., 4.)).unwrap() < 1e-12);
    assert!(
        arc.end()
            .unwrap()
            .distance_to(p(2., 3. - 8f64.sqrt(), 4. - 8f64.sqrt()))
            .unwrap()
            < 1e-12
    );
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Arc"));
    assert!(app.document.undo().unwrap().is_none());
}
