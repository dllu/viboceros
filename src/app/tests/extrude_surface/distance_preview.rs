use super::super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::*;

#[test]
fn surface_extrusion_distance_preview_uses_the_hovered_axis_and_options() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.grid_snap = false;
    let id = surface_at(&mut app, 2., 0.);
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtrudeSrf DeleteInput=Yes");
    let ctx = egui::Context::default();
    let hover = |app: &mut VibocerosApp| {
        let target = point(1., 1.5, 6.);
        for _ in 0..8 {
            let pointer = screen_point(app, target);
            let lines = viewport_frame(app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
            if screen_point(app, target) == pointer {
                return (pointer, lines);
            }
        }
        panic!("viewport layout did not settle");
    };
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(&mut app, &ctx, vec![]);
    let (_, one_side) = hover(&mut app);
    let preview_height = app.surface_extrusion_preview().unwrap().hover.unwrap();
    assert!((preview_height - 4.).abs() < 1e-5);
    let rim = |app: &VibocerosApp, strokes: &[[egui::Pos2; 2]], height| {
        let expected = screen_point(app, point(0., 0., height));
        strokes
            .iter()
            .flatten()
            .any(|p| p.distance(expected) < 1e-3)
    };
    assert!(rim(&app, &one_side, 6.));
    assert!(!rim(&app, &one_side, -2.));

    enter(&mut app, "BothSides");
    let (_, both_sides) = hover(&mut app);
    assert!(rim(&app, &both_sides, 6.));
    assert!(rim(&app, &both_sides, -2.));
    enter(&mut app, "Solid");
    let (_, walls) = hover(&mut app);
    assert!(walls.len() < both_sides.len());
    assert_eq!(format!("{:?}", app.document), before);

    enter(&mut app, "Solid");
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(&mut app, &ctx, vec![]);
    let (pointer, _) = hover(&mut app);
    viewport_click(&mut app, &ctx, pointer);
    if cfg!(feature = "native-smlib") {
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert!(app.document.object(id).is_none());
        let Geometry::Brep(cut) = app.document.selected_objects().next().unwrap().geometry() else {
            panic!("expected a solid sweep");
        };
        assert!(cut.is_solid());
        assert!(cut.bounds().min().distance_to(point(0., 0., -2.)).unwrap() < 1e-5);
        assert!((cut.bounds().max().z() - 6.).abs() < 1e-5);
        assert_eq!(app.document.undo_label(), Some("ExtrudeSrf"));
        enter(&mut app, "Undo");
    } else {
        assert!(app.active_command.is_some());
    }
    assert!(app.document.object(id).is_some());
}

#[test]
fn surface_extrusion_end_on_mouse_distance_matches_the_preview() {
    for (view, target, expected) in [
        (ViewKind::Top, point(1., 4.5, 2.), 3.),
        (ViewKind::Top, point(1., -1.5, 2.), -3.),
        (ViewKind::Top, point(4., 1.5, 2.), 3.),
        (ViewKind::Top, point(4., 4.5, 2.), 18_f64.sqrt()),
        (ViewKind::Bottom, point(1., 4.5, 2.), -3.),
    ] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(view);
        app.active_viewport = 0;
        app.maximized_viewport = Some(0);
        app.osnap = false;
        app.grid_snap = false;
        let id = surface_at(&mut app, 2., 0.);
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        enter(&mut app, "ExtrudeSrf");
        let ctx = egui::Context::default();
        viewport_frame(&mut app, &ctx, vec![]);
        viewport_frame(&mut app, &ctx, vec![]);
        let pointer = screen_point(&app, target);
        assert!(app.viewports[0].screen_rect().unwrap().contains(pointer));
        viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
        let height = app.surface_extrusion_preview().unwrap().hover.unwrap();
        assert!(
            (height - expected).abs() < 1e-5,
            "height {height} for {target:?}"
        );
        assert!(app.document.undo_label().is_none());
        viewport_click(&mut app, &ctx, pointer);
        if cfg!(feature = "native-smlib") {
            assert!(app.active_command.is_none(), "{:?}", app.command_log);
            let Geometry::Brep(cut) = app.document.selected_objects().next().unwrap().geometry()
            else {
                panic!("expected a surface extrusion");
            };
            let shifted = 2. + expected;
            assert!((cut.bounds().min().z() - shifted.min(2.)).abs() < 1e-5);
            assert!((cut.bounds().max().z() - shifted.max(2.)).abs() < 1e-5);
        } else {
            assert!(app.active_command.is_some());
        }
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_distance_points_use_the_source_bounds_and_native_guide_stages() {
    for (steps, warp, expected_max) in [
        (&["w80,70,6"][..], 0., [2., 3., 6.]),
        (&["0"][..], 0., [2., 3., 5.]),
        (
            &["SetBasePoint", "w1,1.5,3", "w99,77,7"][..],
            0.,
            [2., 3., 6.],
        ),
        (
            &["Direction", "w2,3,4", "w3,3,6", "w3,3,6"][..],
            0.,
            [4., 3., 6.],
        ),
        (
            &["Direction", "w2,3,4", "w3,3,6", "w4,11,8"][..],
            0.,
            [5., 3., 8.],
        ),
        (&["w50,60,6"][..], 1., [2., 3., 6.5]),
    ] {
        let mut app = test_app();
        app.active_viewport = 0;
        app.viewports[0] = Viewport::new(ViewKind::Top);
        let plane = app.viewports[0]
            .construction_plane()
            .with_origin(point(10., 20., 5.));
        app.viewports[0].set_construction_plane(plane);
        let id = surface_at(&mut app, 2., warp);
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "ExtrudeSrf Solid=Yes DeleteInput=Yes");
        for (i, step) in steps.iter().enumerate() {
            enter(&mut app, step);
            if i + 1 < steps.len() {
                assert!(
                    app.active_command.is_some(),
                    "{steps:?}: {:?}",
                    app.command_log
                );
                assert!(app.document.object(id).is_some());
                assert_eq!(app.document.objects().count(), 1);
            }
        }
        assert!(
            app.active_command.is_none(),
            "{steps:?}: {:?}",
            app.command_log
        );
        assert!(app.document.object(id).is_none());
        let Geometry::Brep(result) = app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("expected a surface extrusion");
        };
        assert!(
            result.bounds().max().is_near(
                Point3::try_from(expected_max).unwrap(),
                app.document.tolerance()
            ),
            "{steps:?}: {:?}",
            result.bounds()
        );
    }

    let mut app = test_app();
    let id = surface_at(&mut app, 2., 0.);
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrf");
    enter(&mut app, "w0");
    let Geometry::Brep(cut) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!("expected a surface extrusion to the origin");
    };
    assert!(cut.bounds().min().z().abs() < 1e-9);
    assert!((cut.bounds().max().z() - 2.).abs() < 1e-9);
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_typed_distance_uses_the_live_side() {
    for (distance, expected) in [("2", [0., 2.]), ("-2", [2., 4.])] {
        let mut app = test_app();
        let id = surface_at(&mut app, 2., 0.);
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "ExtrudeSrf");
        assert!(app.observe_surface_extrusion_hover(Some(point(1., 1.5, -1.))));
        enter(&mut app, distance);
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert_eq!(
            app.commands.surface_extrusion_settings().distance,
            Some(distance.parse().unwrap())
        );
        let Geometry::Brep(cut) = app.document.selected_objects().next().unwrap().geometry() else {
            panic!("expected a surface extrusion");
        };
        assert!((cut.bounds().min().z() - expected[0]).abs() < 1e-9);
        assert!((cut.bounds().max().z() - expected[1]).abs() < 1e-9);
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_repeats_the_last_successful_distance_and_toggles() {
    let mut app = test_app();
    let id = surface_at(&mut app, 2., 0.);
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(
        &mut app,
        "ExtrudeSrf BothSides=Yes Solid=No DeleteInput=Yes SplitAtTangents=Yes",
    );
    for input in ["SetBasePoint", "w1,1.5,3", "w1,1.5,7"] {
        enter(&mut app, input);
    }
    let assert_result = |app: &VibocerosApp| {
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert!(app.document.object(id).is_none());
        let Geometry::Brep(result) = app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("expected the extruded walls");
        };
        assert!(!result.is_solid());
        assert_eq!(result.faces().len(), 4);
        assert!((result.bounds().min().z() + 2.).abs() < 1e-9);
        assert!((result.bounds().max().z() - 6.).abs() < 1e-9);
    };
    assert_result(&app);
    let saved = app.commands.surface_extrusion_settings();
    assert_eq!(saved.distance, Some(4.));
    assert!(
        saved.options.both_sides && saved.options.delete_input && saved.options.split_at_tangents
    );
    assert!(!saved.options.solid);

    enter(&mut app, "Undo");
    assert!(app.document.object(id).is_some());
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrf");
    assert_eq!(
        app.surface_extrusion_preview().unwrap().constraint.anchor,
        point(1., 1.5, 2.)
    );
    enter(&mut app, "BothSides Solid DeleteInput SplitAtTangents");
    let Some(InteractiveCommand::ExtrudeSurface { options, .. }) = app.active_command else {
        panic!("expected the active distance getter");
    };
    assert!(options.solid);
    assert!(!options.both_sides && !options.delete_input && !options.split_at_tangents);
    enter(&mut app, "Cancel");
    assert_eq!(app.commands.surface_extrusion_settings(), saved);
    assert!(app.document.can_redo());
    enter(&mut app, "ExtrudeSrf");
    assert!(app.command_log.back().unwrap().contains("<4>"));
    enter(&mut app, "");
    assert_result(&app);
}
