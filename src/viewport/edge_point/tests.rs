use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

#[test]
fn screen_queries_follow_off_plane_curves_in_parallel_and_perspective_views() {
    let curve = NurbsCurve::try_new(
        2,
        vec![p(-4., 0., 7.), p(0., 6., 9.), p(4., 0., 11.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Perspective,
    ] {
        let mut view = Viewport::new(kind);
        view.target = NaVector3::new(0., 0., 9.);
        for t in [0.125, 0.375, 0.75] {
            let expected = curve.evaluate(t).unwrap();
            let pointer = view.project(expected, rect()).unwrap();
            assert!(
                rect().contains(pointer),
                "test target must be visible: {kind:?}"
            );
            let actual = view
                .pick_edge_parameter(&curve, pointer, rect(), false)
                .unwrap();
            assert!((actual - t).abs() < 2e-6, "{kind:?}: {t} -> {actual}");
            assert!(
                view.project(curve.evaluate(actual).unwrap(), rect())
                    .unwrap()
                    .distance(pointer)
                    < 0.001
            );
        }
    }
}

#[test]
fn endpoint_osnap_capture_is_separate_from_continuous_edge_location() {
    let curve = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 7.), p(10., 0., 7.)],
        vec![0., 0., 10., 10.],
    )
    .unwrap();
    let view = Viewport::new(ViewKind::Top);
    let pointer = view.project(p(0., 0., 7.), rect()).unwrap() + Vec2::new(3., 0.);
    assert_eq!(
        view.pick_edge_parameter(&curve, pointer, rect(), true),
        Some(0.)
    );
    assert!(
        view.pick_edge_parameter(&curve, pointer, rect(), false)
            .unwrap()
            > 0.
    );
    // A selected edge constrains clicks away from its screen locus too.
    assert!(
        (view
            .pick_edge_parameter(&curve, pointer + Vec2::new(0., 90.), rect(), false)
            .unwrap()
            - view
                .pick_edge_parameter(&curve, pointer, rect(), false)
                .unwrap())
        .abs()
            < 1e-6
    );
    assert!(
        view.pick_edge_parameter(&curve, Pos2::new(Real::NAN as f32, 0.), rect(), false)
            .is_none()
    );
}

#[test]
fn real_pointer_events_emit_the_edge_parameter_without_a_cplane_point() {
    let document = Document::default();
    let curve =
        NurbsCurve::try_new(1, vec![p(-4., 0., 7.), p(4., 0., 7.)], vec![0., 0., 1., 1.]).unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    let context = egui::Context::default();
    let mut frame = |events| {
        let mut output = ViewportOutput::default();
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(rect()),
                    events,
                    ..Default::default()
                },
                |ui| {
                    output = view.show(
                        ui,
                        &document,
                        ViewportInput {
                            edge_curve: Some(&curve),
                            drafting: DraftingInput {
                                active: true,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        &[],
                        0,
                        true,
                    );
                },
            )
            .drop_without_applying_deltas();
        (output, view.last_rect.unwrap())
    };
    let (_, area) = frame(vec![]);
    let pointer = Viewport::new(ViewKind::Top)
        .project(curve.evaluate(0.375).unwrap(), area)
        .unwrap();
    let button = |pressed| egui::Event::PointerButton {
        pos: pointer,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(vec![egui::Event::PointerMoved(pointer), button(true)]);
    let (output, _) = frame(vec![button(false)]);
    assert!((output.edge_parameter.unwrap() - 0.375).abs() < 1e-8);
    assert!(output.picked_point.is_none());
    assert!(output.selection_click.is_none());
    assert!(output.selection_window.is_none());
    assert!(output.edge_click.is_none());
}

#[test]
fn distance_candidates_use_screen_proximity_even_outside_normal_curve_capture() {
    let curve = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 7.), p(10., 0., 7.)],
        vec![0., 0., 10., 10.],
    )
    .unwrap();
    let view = Viewport::new(ViewKind::Top);
    let pointer = view.project(p(9., 0., 7.), rect()).unwrap();
    assert_eq!(
        view.pick_edge_distance_parameter(&curve, &[2., 6.], pointer, rect()),
        Some(6.)
    );
    assert_eq!(
        view.pick_edge_distance_parameter(&curve, &[4.], pointer, rect()),
        Some(4.)
    );
    assert_eq!(
        view.pick_edge_distance_parameter(&curve, &[], pointer, rect()),
        None
    );
    assert_eq!(
        view.pick_edge_distance_parameter(&curve, &[4.], Pos2::new(-1., 0.), rect()),
        None
    );
}

#[test]
fn real_distance_pointer_events_emit_cached_candidate_not_raw_cursor_parameter() {
    let document = Document::default();
    let curve =
        NurbsCurve::try_new(1, vec![p(-4., 0., 7.), p(4., 0., 7.)], vec![0., 0., 1., 1.]).unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    let context = egui::Context::default();
    let mut frame = |events| {
        let mut output = ViewportOutput::default();
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(rect()),
                    events,
                    ..Default::default()
                },
                |ui| {
                    output = view.show(
                        ui,
                        &document,
                        ViewportInput {
                            edge_curve: Some(&curve),
                            edge_distance_parameters: Some(&[0.25]),
                            drafting: DraftingInput {
                                active: true,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        &[],
                        0,
                        true,
                    );
                },
            )
            .drop_without_applying_deltas();
        (output, view.last_rect.unwrap())
    };
    let (_, area) = frame(vec![]);
    let pointer = Viewport::new(ViewKind::Top)
        .project(curve.evaluate(0.875).unwrap(), area)
        .unwrap();
    let button = |pressed| egui::Event::PointerButton {
        pos: pointer,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(vec![egui::Event::PointerMoved(pointer), button(true)]);
    let (output, _) = frame(vec![button(false)]);
    assert_eq!(output.edge_parameter, Some(0.25));
    assert!(output.picked_point.is_none());
    assert!(output.selection_click.is_none());
}

#[test]
fn off_edge_point_snaps_constrain_in_model_space_in_all_four_views() {
    use viboceros_drafting::ObjectSnapKind;
    let curve = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 7.), p(10., 0., 7.)],
        vec![0., 0., 10., 10.],
    )
    .unwrap();
    let mut document = Document::default();
    let target = p(3., -2., 9.);
    let id = document.add_geometry(Geometry::Point(target)).unwrap();
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Perspective,
    ] {
        let mut view = Viewport::new(kind);
        view.target = NaVector3::new(5., 0., 7.);
        let pointer = view.project(target, rect()).unwrap() + Vec2::new(5., 0.);
        assert!(rect().contains(pointer));
        for _ in 0..10 {
            let cursor = view
                .edge_point_cursor(&curve, None, pointer, rect(), &document, true)
                .unwrap();
            assert!((cursor.parameter - 3.).abs() < 1e-12, "{kind:?}");
            let snap = cursor.snap.unwrap();
            assert_eq!(
                (snap.object_id(), snap.kind(), snap.point()),
                (id, ObjectSnapKind::Point, target)
            );
        }
        assert_eq!(view.edge_snap_queries.get(), 1); // Stationary redraws do not solve again.
        assert!(
            view.edge_point_cursor(&curve, None, pointer, rect(), &document, false)
                .unwrap()
                .snap
                .is_none()
        );
    }
}

#[test]
fn feature_kinds_share_capture_and_hidden_objects_cannot_supply_stale_snap_points() {
    use viboceros_drafting::ObjectSnapKind;
    use viboceros_geometry::LineSegment;
    let tolerance = Tolerance::DEFAULT;
    let curve = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 7.), p(10., 0., 7.)],
        vec![0., 0., 10., 10.],
    )
    .unwrap();
    let line = LineSegment::try_new(p(3., -2., 9.), p(3., -6., 9.), tolerance).unwrap();
    let circle = Circle3::try_new(
        p(4., -3., 9.),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        tolerance,
    )
    .unwrap();
    let nonuniform = NurbsCurve::try_new(
        2,
        vec![p(2., -2., 9.), p(3., -2., 9.), p(8., -2., 9.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert_eq!(nonuniform.evaluate(0.5).unwrap(), p(4., -2., 9.));
    let nonuniform_face = Brep::try_surface_face(
        NurbsSurface::try_new(
            2,
            1,
            3,
            2,
            vec![
                p(2., -2., 9.),
                p(3., -2., 9.),
                p(8., -2., 9.),
                p(2., -6., 9.),
                p(3., -6., 9.),
                p(8., -6., 9.),
            ],
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap(),
        tolerance,
    )
    .unwrap();
    for (source, point, kind) in [
        (Geometry::Line(line), p(3., -2., 9.), ObjectSnapKind::End),
        (Geometry::Line(line), p(3., -4., 9.), ObjectSnapKind::Mid),
        (
            Geometry::NurbsCurve(nonuniform),
            p(5., -2., 9.),
            ObjectSnapKind::Mid,
        ),
        (
            Geometry::Brep(nonuniform_face),
            p(5., -2., 9.),
            ObjectSnapKind::Mid,
        ),
        (
            Geometry::Circle(circle),
            circle.center(),
            ObjectSnapKind::Center,
        ),
        (
            Geometry::Circle(circle),
            circle.quadrants().unwrap()[0],
            ObjectSnapKind::Quad,
        ),
    ] {
        let mut document = Document::default();
        let id = document.add_geometry(source).unwrap();
        let mut view = Viewport::new(ViewKind::Top);
        view.target = NaVector3::new(5., 0., 7.);
        let pointer = view.project(point, rect()).unwrap() + Vec2::new(5., 0.);
        let cursor = view
            .edge_point_cursor(&curve, None, pointer, rect(), &document, true)
            .unwrap();
        assert_eq!(cursor.snap.unwrap().kind(), kind);
        assert!((cursor.parameter - point.x()).abs() < 1e-12);
        document.set_objects_locked([id], true).unwrap();
        assert!(
            view.edge_point_cursor(&curve, None, pointer, rect(), &document, true)
                .unwrap()
                .snap
                .is_some()
        );
        document.set_objects_visibility([id], false).unwrap();
        assert!(
            view.edge_point_cursor(&curve, None, pointer, rect(), &document, true)
                .unwrap()
                .snap
                .is_none()
        );
    }
}

#[test]
fn snap_cache_invalidates_for_curve_target_and_tolerance_changes() {
    let curve = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 7.), p(10., 0., 7.)],
        vec![0., 0., 10., 10.],
    )
    .unwrap();
    let view = Viewport::new(ViewKind::Top);
    let target = p(3., -2., 9.);
    assert_eq!(
        view.edge_snap_parameter(&curve, target, Tolerance::DEFAULT),
        Some(3.)
    );
    assert_eq!(
        view.edge_snap_parameter(&curve.clone(), target, Tolerance::DEFAULT),
        Some(3.)
    );
    assert_eq!(view.edge_snap_queries.get(), 1);
    let changed = curve.try_reparameterized(0.0..=20.).unwrap();
    assert_eq!(
        view.edge_snap_parameter(&changed, target, Tolerance::DEFAULT),
        Some(6.)
    );
    assert_eq!(view.edge_snap_queries.get(), 2);
    assert_eq!(
        view.edge_snap_parameter(&changed, p(4., -2., 9.), Tolerance::DEFAULT),
        Some(8.)
    );
    assert_eq!(view.edge_snap_queries.get(), 3);
    let tolerance = Tolerance::try_new(0.01, 1e-12, 1e-10).unwrap();
    assert_eq!(
        view.edge_snap_parameter(&changed, p(4., -2., 9.), tolerance),
        Some(8.)
    );
    assert_eq!(view.edge_snap_queries.get(), 4);
}

#[test]
fn distance_snap_uses_model_space_even_when_screen_space_prefers_other_candidate() {
    let curve = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 0.), p(10., 0., 0.)],
        vec![0., 0., 10., 10.],
    )
    .unwrap();
    let mut view = Viewport::new(ViewKind::Perspective);
    view.target = NaVector3::new(5., 0., 0.);
    for (target, expected, raw) in [(p(4., 5., 0.), 3., 7.), (p(6., -5., 0.), 7., 3.)] {
        let mut document = Document::default();
        document.add_geometry(Geometry::Point(target)).unwrap();
        let pointer = view.project(target, rect()).unwrap();
        let cursor = view
            .edge_point_cursor(&curve, Some(&[3., 7.]), pointer, rect(), &document, true)
            .unwrap();
        assert_eq!(cursor.parameter, expected);
        assert!(cursor.snap.is_some());
        assert_eq!(
            view.pick_edge_distance_parameter(&curve, &[3., 7.], pointer, rect()),
            Some(raw)
        );
        assert_eq!(view.edge_snap_queries.get(), 0); // Distance candidates need no closest-curve solve.
        assert!(
            view.edge_point_cursor(&curve, Some(&[]), pointer, rect(), &document, true)
                .is_none()
        );
    }
}

#[test]
fn real_off_edge_snap_click_subdivides_original_edge_without_selecting_or_moving_the_target() {
    let mut document = Document::default();
    let brep = Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 10.], [0., 12.], [0., 14.]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Brep(brep)).unwrap();
    let point = p(3., -2., 2.);
    let target = document.add_geometry(Geometry::Point(point)).unwrap();
    let target_before = document.object(target).unwrap().clone();
    let source_before = document.object(source).unwrap().clone();
    let mut selection =
        viboceros_command::SplitEdgeSelection::prepare(&document, source, 0).unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    view.target = NaVector3::new(5., 0., 0.);
    view.plane.set(Viewport::default_plane(ViewKind::Front)); // Edge-on CPlane must be irrelevant.
    let context = egui::Context::default();
    let output = {
        let mut frame = |events, active: bool| {
            let mut output = ViewportOutput::default();
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(rect()),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        output = view.show(
                            ui,
                            &document,
                            ViewportInput {
                                edge_curve: active.then(|| selection.curve()),
                                drafting: DraftingInput {
                                    active,
                                    osnap: true,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                            &[],
                            0,
                            true,
                        );
                    },
                )
                .drop_without_applying_deltas();
            (output, view.last_rect.unwrap())
        };
        let (_, area) = frame(vec![], true);
        let mut camera = Viewport::new(ViewKind::Top);
        camera.target = NaVector3::new(5., 0., 0.);
        let pointer = camera.project(point, area).unwrap() + Vec2::new(5., 0.);
        let button = |pressed| egui::Event::PointerButton {
            pos: pointer,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![egui::Event::PointerMoved(pointer), button(true)], true);
        let (output, _) = frame(vec![button(false)], true);
        frame(vec![], false);
        output
    };
    assert!(view.edge_snap_cache.borrow().is_none()); // Do not retain geometry after leaving the prompt.
    assert!((output.edge_parameter.unwrap() - 3.).abs() < 1e-12);
    assert!(
        output.picked_point.is_none()
            && output.selection_click.is_none()
            && output.edge_click.is_none()
    );
    assert_eq!(document.object(source).unwrap(), &source_before);
    selection
        .add_parameter(output.edge_parameter.unwrap())
        .unwrap();
    selection.commit(&mut document).unwrap();
    assert_eq!(document.object(target).unwrap(), &target_before);
    let Geometry::Brep(after) = document.object(source).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(after.edges().len(), 13);
    assert_eq!(after.vertices()[8].point(), p(3., 0., 0.));
    document.undo().unwrap();
    assert_eq!(document.object(source).unwrap(), &source_before);
    assert_eq!(document.object(target).unwrap(), &target_before);
}
