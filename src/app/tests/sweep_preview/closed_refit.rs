use super::*;

#[test]
fn sweep_refit_matches_and_previews_regions_of_scaled_closed_profiles() {
    let mut app = test_app();
    for input in [
        "Line 0,0,0 0,0,5",
        "Curve 0,0,0 2,2,0 -2,2,0 0,0,0 Degree=3",
        "Curve 0,0,5 1.3,1.3,5 -1.3,1.3,5 0,0,5 Degree=3",
    ] {
        app.commands.execute(&mut app.document, input).unwrap();
    }
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let context = egui::Context::default();

    enter(&mut app, "Sweep1 Refit=0.01");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    enter(&mut app, "Natural");
    enter(&mut app, "");
    assert!(app.command_hint().starts_with("Review the sweep"));

    let timeout = Duration::from_secs(20);
    let loose = wait_for_wires_with_timeout(&mut app, &context, timeout, |wires| {
        wires.iter().any(|wire| wire.control_points().len() == 12)
    });
    enter(&mut app, "RefitTolerance=0.001");
    let tight = wait_for_wires_with_timeout(&mut app, &context, timeout, |wires| {
        wires.iter().any(|wire| wire.control_points().len() == 16)
    });
    assert_ne!(loose, tight);
    assert_eq!(app.document.undo_label(), None);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);

    enter(&mut app, "");
    let Geometry::Brep(output) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected the matched closed sweep to commit");
    };
    assert_eq!(
        output
            .wireframe_curves(1, app.document.tolerance())
            .unwrap(),
        tight
    );
    assert_eq!(output.faces().len(), 1);
    let surface = output.faces()[0].surface();
    assert_eq!(
        (surface.degree_u(), surface.control_point_count_v()),
        (1, 16)
    );
    assert!((surface.domain_v().end() - 4.073901366479867).abs() < 1e-7);
    let handle = surface.control_point(0, 1).unwrap().point();
    assert!(
        handle
            .distance_to(point(0.21214359860376428, 0.21214359860376428, 0.))
            .unwrap()
            < 1e-7
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}
