use super::*;

fn review_closed_sections(commands: [&str; 3], sweep: &str) -> (VibocerosApp, egui::Context) {
    let mut app = test_app();
    for input in commands {
        app.commands.execute(&mut app.document, input).unwrap();
    }
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    enter(&mut app, sweep);
    for id in ids {
        enter(&mut app, &id.to_string());
    }
    enter(&mut app, "");
    enter(&mut app, "Natural");
    enter(&mut app, "");
    assert!(app.command_hint().starts_with("Review the sweep"));
    (app, egui::Context::default())
}

#[test]
fn sweep_refit_matches_and_previews_regions_of_scaled_closed_profiles() {
    let (mut app, context) = review_closed_sections(
        [
            "Line 0,0,0 0,0,5",
            "Curve 0,0,0 2,2,0 -2,2,0 0,0,0 Degree=3",
            "Curve 0,0,5 1.3,1.3,5 -1.3,1.3,5 0,0,5 Degree=3",
        ],
        "Sweep1 Refit=0.01",
    );
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
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

#[test]
fn sweep_refit_previews_polygon_corners_with_a_closed_loop() {
    let (mut app, context) = review_closed_sections(
        [
            "Line 0,0,0 0,0,5",
            "Polyline 0,0,0 2,0,0 2,1,0 0,1,0 0,0,0",
            "Curve 0,0,5 2,2,5 -2,2,5 0,0,5 Degree=3",
        ],
        "Sweep1 Refit=0.01 RefitRail=No",
    );
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let timeout = Duration::from_secs(20);
    let coarse =
        wait_for_wires_with_timeout(&mut app, &context, timeout, |wires| !wires.is_empty());
    enter(&mut app, "RefitTolerance=0.001");
    let tight = wait_for_wires_with_timeout(&mut app, &context, timeout, |wires| {
        !wires.is_empty() && wires != coarse
    });
    assert_eq!(app.document.undo_label(), None);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);

    enter(&mut app, "");
    let Geometry::Brep(output) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected the polygon Sweep1 draft to commit");
    };
    assert_eq!(
        output
            .wireframe_curves(1, app.document.tolerance())
            .unwrap(),
        tight
    );
    let mut faces: Vec<_> = output.faces().iter().map(|face| face.surface()).collect();
    faces.sort_by(|a, b| a.domain_v().start().total_cmp(b.domain_v().start()));
    assert_eq!(faces.len(), 4);
    for (face, (start, end, controls)) in
        faces
            .iter()
            .zip([(0., 2., 6), (2., 3., 6), (3., 5., 6), (5., 6., 4)])
    {
        assert_eq!(
            (face.degree_u(), face.control_point_count_v()),
            (1, controls)
        );
        let domain = face.domain_v();
        assert!((domain.start() - start).abs() < 1e-7);
        assert!((domain.end() - end).abs() < 1e-7);
    }
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}

#[test]
fn sweep_refit_keeps_the_matched_section_domain_with_both_point_ends() {
    let (mut app, context) = review_closed_sections(
        [
            "Line 0,0,0 0,0,10",
            "Polyline 0,0,2.5 2,0,2.5 2,1,2.5 0,1,2.5 0,0,2.5",
            "Curve 0,0,7.5 2,2,7.5 -2,2,7.5 0,0,7.5 Degree=3",
        ],
        "Sweep1 Refit=0.001",
    );
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let timeout = Duration::from_secs(20);
    let original =
        wait_for_wires_with_timeout(&mut app, &context, timeout, |wires| !wires.is_empty());
    for (getter, coordinates, end) in [
        ("StartPoint", "w0,0,0", "start"),
        ("EndPoint", "w0,0,10", "end"),
    ] {
        enter(&mut app, getter);
        assert!(
            app.command_hint()
                .starts_with(&format!("Pick the Sweep1 {end} point"))
        );
        enter(&mut app, coordinates);
    }
    let adapted = wait_for_wires_with_timeout(&mut app, &context, timeout, |wires| {
        !wires.is_empty() && wires != original
    });
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
    enter(&mut app, "");
    let Geometry::Brep(output) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected the two-point-ended Sweep1 draft to commit");
    };
    assert_eq!(
        output
            .wireframe_curves(1, app.document.tolerance())
            .unwrap(),
        adapted
    );
    let mut boundaries = vec![];
    for face in output.faces() {
        let surface = face.surface();
        let u = surface.domain_u();
        let v = surface.domain_v();
        assert_eq!(surface.control_point_count_u(), 10);
        let mid = v.start().midpoint(*v.end());
        assert!(
            surface
                .evaluate(*u.start(), mid)
                .unwrap()
                .distance_to(point(0., 0., 0.))
                .unwrap()
                < 1e-7
        );
        assert!(
            surface
                .evaluate(*u.end(), mid)
                .unwrap()
                .distance_to(point(0., 0., 10.))
                .unwrap()
                < 1e-7
        );
        boundaries.push(*v.end());
    }
    boundaries.sort_by(f64::total_cmp);
    assert_eq!(boundaries.len(), 4);
    assert!(
        boundaries
            .iter()
            .zip([2., 3., 5., 6.])
            .all(|(actual, expected)| (actual - expected).abs() < 1e-7)
    );
}
