use super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::*;
use viboceros_command::CurvatureSample;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn view(app: &mut VibocerosApp) -> egui::Context {
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.osnap = false;
    let context = egui::Context::default();
    for _ in 0..3 {
        viewport_frame(app, &context, Vec::new());
    }
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    viewport_frame(app, &context, Vec::new());
    context
}

fn hover(
    app: &mut VibocerosApp,
    context: &egui::Context,
    target: Point3,
    offset: egui::Vec2,
) -> (egui::Pos2, CurvatureSample) {
    for _ in 0..6 {
        let pixel = screen_point(app, target) + offset;
        viewport_frame(app, context, vec![egui::Event::PointerMoved(pixel)]);
        if screen_point(app, target) + offset == pixel
            && let Some(guide) = app.curvature_session.as_ref().and_then(|s| s.guides())
        {
            return (pixel, guide.sample);
        }
    }
    panic!("No curvature sample at {target:?}: {:?}", app.command_log);
}

#[test]
fn curvature_curve_source_and_repeated_picks_use_the_visible_circles() {
    let mut app = test_app();
    let xy = viboceros_command::CommandContext::default().construction_plane;
    let ellipse = viboceros_geometry::Ellipse3::try_new(
        point(0., 0., 0.),
        4.,
        2.,
        xy.x_axis(),
        xy.y_axis(),
        app.document.tolerance(),
    )
    .unwrap();
    app.document
        .add_geometry(Geometry::Ellipse(ellipse))
        .unwrap();
    app.document.clear_history().unwrap();
    let ctx = view(&mut app);
    enter(&mut app, "Curvature");
    for _ in 0..3 {
        viewport_frame(&mut app, &ctx, Vec::new());
    }
    assert!(app.selecting_curvature_source());
    let source_pixel = screen_point(&app, point(4., 0., 0.));
    viewport_click(&mut app, &ctx, source_pixel);
    assert!(!app.selecting_curvature_source());
    assert_eq!(app.document.selected_object_count(), 0);
    let features = app
        .curvature_session
        .as_ref()
        .unwrap()
        .input()
        .unwrap()
        .features;
    assert_eq!(features.len(), 2);
    assert!(
        features
            .iter()
            .all(|feature| { !feature.inflection && (feature.point.x().abs() - 4.).abs() < 1e-10 })
    );

    let mut expected = Vec::new();
    for (target, radius, offset) in [
        (point(4., 0., 0.), 1., egui::vec2(0., 4.)),
        (point(0., 2., 0.), 8., egui::Vec2::ZERO),
    ] {
        let (mut pixel, sample) = hover(&mut app, &ctx, target, offset);
        let CurvatureSample::Curve { curvature, .. } = sample else {
            panic!("curve")
        };
        assert!((1. / curvature.length().unwrap() - radius).abs() < 1e-7);
        assert!(!viewport_frame(&mut app, &ctx, Vec::new()).is_empty());
        if expected.is_empty() {
            viewport_click(&mut app, &ctx, pixel);
            assert!(app.curvature_session.is_some());
            assert!(!app.document.can_undo());
            assert_eq!(app.document.objects().count(), 1);
            enter(&mut app, "MarkCurvature");
            pixel = hover(&mut app, &ctx, target, offset).0;
        }
        let guide = app.curvature_session.as_ref().unwrap().guides().unwrap();
        expected.extend_from_slice(guide.markers);
        viewport_click(&mut app, &ctx, pixel);
        assert!(app.curvature_session.is_some());
    }
    enter(&mut app, "");
    assert!(app.command_line_idle());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(
        app.document
            .objects()
            .skip(1)
            .map(|o| o.geometry().clone())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(app.document.undo_label(), Some("Curvature"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().count(), 1);
    assert!(!app.document.can_undo());
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 5);
    enter(&mut app, "Curvature");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Curvature { mark: true })
    );
}

#[test]
fn curvature_requires_one_interactive_source_and_ignores_another_curve() {
    let mut app = test_app();
    for input in ["Circle w0,0,0 2", "Circle w10,0,0 2", "SelAll"] {
        app.execute_command(input);
    }
    let source = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    let ctx = view(&mut app);
    enter(&mut app, "Curvature MarkCurvature=Yes");
    assert!(app.selecting_curvature_source());
    enter(&mut app, &format!("SelID {source}"));
    assert!(!app.selecting_curvature_source());
    assert_eq!(app.document.selected_object_count(), 0);
    hover(&mut app, &ctx, point(2., 0., 0.), egui::Vec2::ZERO);
    let other = screen_point(&app, point(8., 0., 0.));
    viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(other)]);
    assert!(app.curvature_status().is_none());
    viewport_click(&mut app, &ctx, other);
    assert_eq!(app.document.objects().count(), 2);
    assert!(!app.document.can_undo());
}

#[test]
fn curvature_viewport_respects_a_trim_hole() {
    let mut app = test_app();
    for input in [
        "Rectangle w-5,-5,0 w5,5,0",
        "Rectangle w-1,-1,0 w1,1,0",
        "SelAll",
        "PlanarSrf",
    ] {
        app.execute_command(input);
    }
    let owner = app.document.objects().last().unwrap();
    assert!(matches!(owner.geometry(), Geometry::Brep(_)));
    let owner = owner.id();
    app.document
        .select_objects_direct([owner], SelectionMode::Replace)
        .unwrap();
    let count = app.document.objects().count();
    app.document.clear_history().unwrap();
    let ctx = view(&mut app);
    enter(&mut app, "Curvature MarkCurvature=Yes");
    let (_, sample) = hover(&mut app, &ctx, point(3., 0., 0.), egui::Vec2::ZERO);
    assert!(matches!(
        sample,
        CurvatureSample::Surface { face: Some(0), .. }
    ));
    let hole = screen_point(&app, point(0., 0., 0.));
    viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(hole)]);
    assert!(app.curvature_status().is_none());
    viewport_click(&mut app, &ctx, hole);
    assert!(app.curvature_session.is_some());
    assert_eq!(app.document.objects().count(), count);
    assert!(!app.document.can_undo());
}

#[test]
fn curvature_surface_hover_and_click_use_the_same_oriented_principal_guides() {
    for (command, target, curved) in [
        (
            "SrfPt w-4,-3,2 w4,-3,2 w4,3,2 w-4,3,2",
            point(1., 1., 2.),
            false,
        ),
        ("Sphere w0,0,0 2", point(1.2, 0., 1.6), true),
    ] {
        let mut app = test_app();
        app.execute_command(command);
        let source = app.document.objects().next().unwrap().id();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let ctx = view(&mut app);
        enter(&mut app, "Curvature MarkCurvature=Yes");
        let (pixel, sample) = hover(&mut app, &ctx, target, egui::Vec2::ZERO);
        let CurvatureSample::Surface { curvature, .. } = sample else {
            panic!("surface")
        };
        assert!(
            curvature.point.distance_to(target).unwrap() < 2e-6,
            "{curvature:?}"
        );
        for k in curvature.principal {
            assert!(
                (k - if curved { -0.5 } else { 0.0 }).abs() < 1e-10,
                "{curvature:?}"
            );
        }
        let guides = app.curvature_session.as_ref().unwrap().guides().unwrap();
        assert_eq!(guides.markers.len(), 3);
        assert!(guides.markers[1..].iter().all(|marker| {
            if curved {
                matches!(marker, Geometry::Arc(_))
            } else {
                matches!(marker, Geometry::Line(_))
            }
        }));
        let expected = guides.markers.to_vec();
        assert!(!app.document.can_undo());
        assert!(!viewport_frame(&mut app, &ctx, Vec::new()).is_empty());
        viewport_click(&mut app, &ctx, pixel);
        assert!(app.curvature_session.is_some());
        app.cancel_interactive_command(true);
        assert_eq!(
            app.document
                .objects()
                .skip(1)
                .map(|o| o.geometry().clone())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(app.document.is_selected(source));
        app.document.undo().unwrap();
        assert_eq!(app.document.objects().count(), 1);
        assert!(app.document.is_selected(source));
    }
}

#[test]
fn curvature_snaps_to_an_inflection_and_marks_only_the_point() {
    let mut app = test_app();
    let curve = viboceros_geometry::NurbsCurve::try_new(
        3,
        vec![
            point(-3., 0., 0.),
            point(-1., 2., 0.),
            point(1., -2., 0.),
            point(3., 0., 0.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let id = app
        .document
        .add_geometry(Geometry::NurbsCurve(curve))
        .unwrap();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let ctx = view(&mut app);
    enter(&mut app, "Curvature MarkCurvature=Yes");
    let (pixel, sample) = hover(&mut app, &ctx, point(0., 0., 0.), egui::vec2(4., 0.));
    assert!(sample.point().distance_to(point(0., 0., 0.)).unwrap() < 1e-8);
    assert_eq!(app.curvature_status(), Some("Radius = infinite"));
    viewport_click(&mut app, &ctx, pixel);
    assert_eq!(app.document.objects().count(), 2);
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Point(_)
    ));
    enter(&mut app, "");
}
