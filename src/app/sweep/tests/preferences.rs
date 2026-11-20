use super::*;
use viboceros_geometry::CircularArc3;

#[test]
fn sweep_session_shares_defaults_and_distinguishes_normal_and_scripted_cancel() {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let middle = std::f64::consts::FRAC_1_SQRT_2;
    let rail = CircularArc3::try_from_three_points(
        point(1., 0., 0.),
        point(middle, middle, 0.),
        point(0., 1., 0.),
        tolerance,
    )
    .unwrap();
    app.document.add_geometry(Geometry::Arc(rail)).unwrap();
    app.commands
        .execute(&mut app.document, "Line 1,0,0 2,0,0.5")
        .unwrap();
    app.document.clear_history().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "_-Sweep1");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    for input in [
        "Style=Roadlike",
        "w0,0,0",
        "w0,1,1",
        "Simplify=Rebuild",
        "RebuildCount=9",
        "ShapeBlending=Global",
        "RefitRail=Yes",
    ] {
        enter(&mut app, input);
    }
    let oblique = viboceros_geometry::Vector3::try_new(0., 1., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    // Rhino 8.32 keeps accepted options when the scripted command is cancelled.
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert_eq!(app.document.undo_label(), None);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);

    app.restore_curve_selection(sources.iter().map(|source| source.id()).collect());
    enter(&mut app, "Sweep1");
    assert!(app.command_hint().starts_with("Review the sweep"));
    for input in ["Style=Freeform", "RefitRail=No", "RebuildCount=6"] {
        enter(&mut app, input);
    }
    app.cancel_current_prompt_or_selection();

    enter(&mut app, "_Sweep1");
    let options = &app.sweep_session.as_ref().unwrap().state.options;
    assert_eq!(options.axis, Some(oblique));
    assert_eq!(options.global, Some(true));
    assert_eq!(options.refit, Some(true));
    assert_eq!(options.rebuild_point_count, 9);
    enter(&mut app, "Simplify=None");
    enter(&mut app, "RebuildCount=8");
    enter(&mut app, "Undo");
    assert_eq!(
        app.sweep_session
            .as_ref()
            .unwrap()
            .state
            .options
            .rebuild_count(),
        9
    );
    enter(&mut app, "RebuildCount=8");
    enter(&mut app, "RebuildCount");
    assert!(app.command_log.back().unwrap().contains("[8]"));
    enter(&mut app, "");
    assert!(app.command_hint().starts_with("Review the sweep"));
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);

    // A dashed preselected invocation uses the same settings as review.
    enter(&mut app, "_-Sweep1 Simplify=Rebuild");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected Sweep1 surface: {:?}", app.command_log);
    };
    let surface = sweep.faces()[0].surface();
    assert_eq!((surface.degree_u(), surface.degree_v()), (3, 3));
    assert_eq!(surface.control_point_count_v(), 8);
    let upper = surface
        .evaluate(*surface.domain_u().end(), *surface.domain_v().end())
        .unwrap();
    assert!(
        upper
            .distance_to(point(0., 2.060660171779821, -0.35355339059327373))
            .unwrap()
            < 1e-8
    );
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
}
