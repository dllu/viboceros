use super::*;
use viboceros_drafting::ObjectSnapKind;
use viboceros_geometry::{CircularArc3, CurveSegment3, LineSegment, PolyCurve3};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn enabled_modes_reach_both_parallel_and_perspective_capture_queries() {
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
        let mut document = Document::default();
        let arc = CircularArc3::try_from_three_points(
            map(-2., 0.),
            map(0., 2.),
            map(2., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        document.add_geometry(Geometry::Arc(arc)).unwrap();
        let mut view = Viewport::new(kind);
        view.target = NaVector3::from(map(0., 0.).to_array());
        let pointer = view.project(map(2., 0.), area()).unwrap() + Vec2::new(2., 0.);
        for feature in [ObjectSnapKind::End, ObjectSnapKind::Center] {
            let cursor = view
                .drafting_cursor(
                    pointer,
                    area(),
                    &document,
                    DraftingInput {
                        active: true,
                        osnap: ObjectSnapModes::only(feature),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(cursor.object_snap.unwrap().kind(), feature, "{kind:?}");
            assert!(
                cursor
                    .point
                    .distance_to(if feature == ObjectSnapKind::End {
                        map(2., 0.)
                    } else {
                        map(0., 0.)
                    })
                    .unwrap()
                    < 1e-10
            );
        }
        for modes in [
            ObjectSnapModes::NONE,
            ObjectSnapModes::only(ObjectSnapKind::Point),
        ] {
            assert!(
                view.object_snap(pointer, area(), &document, modes)
                    .is_none(),
                "{kind:?}"
            );
        }
    }
}
fn area() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

#[test]
fn ordinary_and_edge_constrained_prompts_share_center_hover_in_all_views() {
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Perspective,
    ] {
        // Align a circle with each parallel view; use XY in Perspective.
        let map = |x, y| match kind {
            ViewKind::Front => p(x, 7., y),
            ViewKind::Right => p(7., x, y),
            _ => p(x, y, 7.),
        };
        let arc = CircularArc3::try_from_three_points(
            map(-2., 0.),
            map(0., 2.),
            map(2., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let composite = PolyCurve3::try_new(vec![
            CurveSegment3::Arc(arc),
            LineSegment::try_new(map(2., 0.), map(5., 0.), Tolerance::DEFAULT)
                .unwrap()
                .into(),
        ])
        .unwrap();
        let mut doc = Document::default();
        let id = doc.add_geometry(Geometry::PolyCurve(composite)).unwrap();
        let mut view = Viewport::new(kind);
        let center = map(0., 0.);
        view.target = NaVector3::from(center.to_array());
        view.plane.set(Viewport::default_plane(ViewKind::Right));
        let pointer = view.project(map(-1.6, 1.2), area()).unwrap() + Vec2::new(2., 0.);
        let input = DraftingInput {
            active: true,
            osnap: viboceros_drafting::ObjectSnapModes::ALL,
            ..Default::default()
        };
        let cursor = view.drafting_cursor(pointer, area(), &doc, input).unwrap();
        assert!(
            cursor.point.distance_to(center).unwrap() < 1e-12,
            "{kind:?}"
        );
        assert_eq!(cursor.object_snap.unwrap().kind(), ObjectSnapKind::Center);
        assert_eq!(cursor.object_snap.unwrap().object_id(), id);
        // Constrained target differs from the captured center, not from the mouse.
        let curve = NurbsCurve::try_new(1, vec![map(-3., -2.), map(3., -2.)], vec![0., 0., 6., 6.])
            .unwrap();
        let edge = view
            .edge_point_cursor(
                &curve,
                None,
                pointer,
                area(),
                &doc,
                viboceros_drafting::ObjectSnapModes::ALL,
            )
            .unwrap();
        assert!((edge.parameter - 3.).abs() < 1e-10, "{kind:?}");
        assert!(
            view.object_snap(
                view.project(center, area()).unwrap(),
                area(),
                &doc,
                ObjectSnapModes::ALL
            )
            .is_none(),
            "{kind:?}"
        );
        doc.set_objects_visibility([id], false).unwrap();
        assert!(
            view.object_snap(pointer, area(), &doc, ObjectSnapModes::ALL)
                .is_none()
        );
    }
}

#[test]
fn real_drafting_click_returns_off_cursor_center_not_construction_plane_intersection() {
    let mut doc = Document::default();
    let arc = CircularArc3::try_from_three_points(
        p(-2., 0., 7.),
        p(0., 2., 7.),
        p(2., 0., 7.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    doc.add_geometry(Geometry::Arc(arc)).unwrap();
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
                                osnap: viboceros_drafting::ObjectSnapModes::ALL,
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
        .project(p(-1.6, 1.2, 7.), rect)
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
            < 1e-12
    );
    assert!(output.selection_click.is_none());
    assert!(output.edge_click.is_none());
}
