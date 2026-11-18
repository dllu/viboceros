use super::*;

#[test]
fn solid_axial_tube_previews_the_second_rim_and_commits_open_walls_together() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.grid_snap = false;
    app.osnap = false;
    let ctx = egui::Context::default();
    for token in ["Tube Solid=No", "w0,0,0", "4"] {
        enter(&mut app, token);
    }
    let input = app.solid_axial_preview_input().unwrap();
    assert_eq!(input.kind, Kind::Tube);
    assert!(input.prompt().contains("WallThickness"));
    assert!(input.height_constraint().is_none());
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(&mut app, &ctx, vec![]);
    let near = screen_point(&app, point(1., 0., 0.));
    let far = screen_point(&app, point(2., 0., 0.));
    let a = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(near)]);
    let b = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(far)]);
    assert!(!a.is_empty());
    assert_ne!(a, b);
    assert_eq!(app.document.objects().len(), 0);
    viewport_click(&mut app, &ctx, far);
    assert!(
        app.solid_axial_preview_input()
            .unwrap()
            .height_constraint()
            .is_some()
    );
    enter(&mut app, "BothSides=Yes");
    app.viewports[0] = Viewport::new(ViewKind::Front);
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(&mut app, &ctx, vec![]);
    let end = screen_point(&app, point(0., 0., 7.));
    let walls = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(walls.len() > b.len());
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &ctx, end);

    assert_eq!(app.document.objects().len(), 2);
    for (object, radius) in app.document.objects().zip([4., 2.]) {
        let Geometry::Brep(wall) = object.geometry() else {
            panic!("expected an open wall");
        };
        assert!(wall.is_extrusion());
        assert!(!wall.is_solid());
        assert_eq!(wall.faces().len(), 1);
        let surface = wall.faces()[0].surface();
        for (row, z) in [-7., 7.].into_iter().enumerate() {
            let center = cylinder_end(surface, row);
            assert!(
                center.distance_to(point(0., 0., z)).unwrap() < 2e-5,
                "{row}: {center:?}, expected z={z}"
            );
            let radial = center
                .distance_to(surface.control_point(0, row).unwrap().point())
                .unwrap();
            assert!((radial - radius).abs() < 2e-5);
        }
    }
    assert!(app.solid_axial_session.is_none());
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Tube"));
    assert_eq!(app.document.objects().len(), 0);
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("Tube"));
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn solid_axial_tube_recovers_from_an_invalid_thickness_and_keeps_the_curve_guide() {
    let mut app = test_app();
    enter(&mut app, "UseExtrusions Polysurface");
    enter(&mut app, "Line w2,3,4 w6,7,12");
    let guide = app.document.objects().next().unwrap().id();
    enter(&mut app, "Tube DirectionConstraint=AroundCurve");
    assert!(app.pick_circle_around_curve(Some(guide), None));
    for token in ["w4,5,8", "5", "WallThickness", "-5"] {
        enter(&mut app, token);
    }
    assert!(
        app.solid_axial_preview_input()
            .unwrap()
            .prompt()
            .starts_with("WallThickness")
    );
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.undo_label(), Some("Line"));
    enter(&mut app, "-2");
    assert!(
        app.solid_axial_preview_input()
            .unwrap()
            .prompt()
            .starts_with("End of tube")
    );
    enter(&mut app, "-5");
    let tube = brep(&app);
    assert!(tube.is_solid());
    assert!(!tube.is_extrusion());
    let endpoint = cylinder_end(tube.faces()[2].surface(), 1);
    let native = point(1.9587585476806844, 2.9587585476806852, 3.9175170953613705);
    assert!(endpoint.distance_to(native).unwrap() < 2e-12);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Tube"));
    assert_eq!(app.document.objects().next().unwrap().id(), guide);
    assert!(!app.document.is_selected(guide));
}

#[test]
fn solid_axial_tube_free_height_locks_the_length_and_waits_for_a_direction() {
    let mut app = test_app();
    for step in [
        "Tube DirectionConstraint=None",
        "w1,2,3",
        "5",
        "w1,4,12",
        "-5",
    ] {
        enter(&mut app, step);
    }
    assert!(!app.document.can_undo());
    let getter = app.solid_axial_preview_input().unwrap();
    assert_eq!(getter.height_constraint().unwrap().distance, Some(5.));
    let direction = point(6., 7., 9.);
    let preview = getter.preview_definition(Some(direction)).unwrap();
    assert_eq!(preview.height, -5.);
    enter(&mut app, "r5,5,6");
    let saved = brep(&app);
    assert!(saved.is_solid());
    let endpoint = cylinder_end(saved.faces()[0].surface(), 1);
    let expected = point(
        -1.6958193300859608,
        -0.6958193300859609,
        -0.2349831961031541,
    );
    assert!(endpoint.distance_to(expected).unwrap() < 2e-12);
    assert!(app.solid_axial_session.is_none());
}

#[test]
fn solid_axial_tube_postselected_fit_continues_to_the_second_wall() {
    let mut app = test_app();
    for token in [
        "Point w50,60,70",
        "Point w47,57,70",
        "Point w50,54,70",
        "Point w53,57,70",
        "Tube FitPoints",
        "SelAll",
    ] {
        enter(&mut app, token);
    }
    assert!(app.solid_axial_fit_preview().is_some());
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    let input = app.solid_axial_preview_input().unwrap();
    assert!(input.prompt().contains("WallThickness"));
    assert_eq!(input.anchor(), Some(point(50., 57., 70.)));
    assert_eq!(app.document.objects().len(), 4);
    enter(&mut app, "2");
    enter(&mut app, "4");
    assert!(brep(&app).is_solid());
    assert_eq!(app.document.objects().len(), 5);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Tube"));
    assert_eq!(app.document.objects().len(), 4);
    assert!(app.document.selected_object_ids().next().is_none());
}
