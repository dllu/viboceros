use super::*;
use viboceros_drafting::ObjectSnapKind;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn area() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

#[test]
fn distant_mid_hover_reaches_ordinary_and_constrained_prompts_in_all_views() {
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Perspective,
    ] {
        let map = |x, y| match kind {
            ViewKind::Front => p(x, 7., y),
            ViewKind::Right => p(7., x, y),
            _ => p(x, y, 7.),
        };
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::NurbsCurve(
                NurbsCurve::try_new(
                    2,
                    vec![map(-5., 0.), map(-3., 0.), map(5., 0.)],
                    vec![0., 0., 0., 1., 1., 1.],
                )
                .unwrap(),
            ))
            .unwrap();
        let mut view = Viewport::new(kind);
        view.target = NaVector3::from(map(0., 0.).to_array());
        view.plane.set(Viewport::default_plane(ViewKind::Right));
        let pointer = view.project(map(-4., 0.), area()).unwrap() + Vec2::new(0., 2.);
        let modes = ObjectSnapModes::only(ObjectSnapKind::Mid);
        let cursor = view
            .drafting_cursor(
                pointer,
                area(),
                &doc,
                DraftingInput {
                    active: true,
                    osnap: modes,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            cursor.object_snap.unwrap().kind(),
            ObjectSnapKind::Mid,
            "{kind:?}"
        );
        assert_eq!(cursor.object_snap.unwrap().object_id(), id);
        assert!(cursor.point.distance_to(map(0., 0.)).unwrap() < 1e-10);
        assert!(
            view.project(cursor.point, area())
                .unwrap()
                .distance(pointer)
                > 50.
        );
        assert!(
            view.object_snap(
                pointer,
                area(),
                &doc,
                modes.with(ObjectSnapKind::Point, true)
            )
            .is_none()
        );
        let edge =
            NurbsCurve::try_new(1, vec![map(-6., -3.), map(6., -3.)], vec![0., 0., 12., 12.])
                .unwrap();
        let constrained = view
            .edge_point_cursor(&edge, None, pointer, area(), &doc, modes)
            .unwrap();
        assert!((constrained.parameter - 6.).abs() < 1e-9, "{kind:?}");
    }
}

#[test]
fn real_pointer_pick_returns_off_cursor_mid_instead_of_a_cplane_point() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Line(
        viboceros_geometry::LineSegment::try_new(p(-5., 0., 7.), p(5., 0., 7.), Tolerance::DEFAULT)
            .unwrap(),
    ))
    .unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    let context = egui::Context::default();
    let mut frame = |events| {
        let mut output = ViewportOutput::default();
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(area()),
                    events,
                    ..Default::default()
                },
                |ui| {
                    output = view.show(
                        ui,
                        &doc,
                        ViewportInput {
                            drafting: DraftingInput {
                                active: true,
                                osnap: ObjectSnapModes::only(ObjectSnapKind::Mid),
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
    let (_, rect) = frame(vec![]);
    let pointer = Viewport::new(ViewKind::Top)
        .project(p(-4., 0., 7.), rect)
        .unwrap();
    let button = |pressed| egui::Event::PointerButton {
        pos: pointer,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(vec![egui::Event::PointerMoved(pointer), button(true)]);
    let (output, _) = frame(vec![button(false)]);
    assert!(
        output
            .picked_point
            .unwrap()
            .distance_to(p(0., 0., 7.))
            .unwrap()
            < 1e-10
    );
    assert!(output.selection_click.is_none() && output.edge_click.is_none());
}
