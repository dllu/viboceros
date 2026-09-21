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
    assert!(
        view.pick_edge_parameter(&curve, pointer + Vec2::new(0., 9.), rect(), false)
            .is_none()
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
