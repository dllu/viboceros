use super::*;
use viboceros_drafting::ObjectSnapKind;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn area() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

#[test]
fn perspective_near_captures_a_thin_visible_part_of_a_camera_crossing_line() {
    for reverse in [false, true] {
        let view = Viewport::new(ViewKind::Perspective);
        let (right, _, forward) = view.perspective_basis();
        let at = |x: Real, depth: Real| {
            let xyz =
                view.target + right * x + forward * (depth - view.perspective_camera_distance);
            Point3::try_new(xyz.x, xyz.y, xyz.z).unwrap()
        };
        let depth = 1e6;
        let a = at(0., 1.);
        let b = at(depth, -depth);
        // Aim a quarter viewport-width to the right of the camera center.
        let screen_ratio =
            Real::from(area().width()) * 0.25 / view.perspective_focal_length_pixels(area());
        let t = screen_ratio / (depth + screen_ratio * (depth + 1.));
        let expected = at(depth * t, 1. - (depth + 1.) * t);
        let pointer = view.project(expected, area()).unwrap();
        assert!(area().contains(pointer));
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    if reverse { b } else { a },
                    if reverse { a } else { b },
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let cursor = view
            .drafting_cursor(
                pointer,
                area(),
                &doc,
                DraftingInput {
                    active: true,
                    osnap: ObjectSnapModes::only(ObjectSnapKind::Near),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(cursor.object_snap.unwrap().object_id(), id);
        // This includes the ordinary egui f32 pointer quantization.
        assert!(cursor.point.distance_to(expected).unwrap() < 1e-5);
        assert!(view.project_precise(cursor.point, area()).is_some());
    }
}

#[test]
fn near_reaches_ordinary_and_edge_constrained_prompts_in_all_four_views() {
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
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    map(-5., 0.),
                    map(5., 0.),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let mut view = Viewport::new(kind);
        view.target = NaVector3::from(map(0., 0.).to_array());
        view.plane.set(Viewport::default_plane(ViewKind::Right));
        let pointer = view.project(map(-3.7, 0.), area()).unwrap();
        let modes = ObjectSnapModes::only(ObjectSnapKind::Near);
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
        assert_eq!(cursor.object_snap.unwrap().kind(), ObjectSnapKind::Near);
        assert_eq!(cursor.object_snap.unwrap().object_id(), id);
        // Input pixels are egui f32; this bound is on cursor quantization,
        // not the f64 geometric/projective solver's accuracy.
        assert!(
            cursor.point.distance_to(map(-3.7, 0.)).unwrap() < 1e-5,
            "{kind:?}"
        );
        let edge =
            NurbsCurve::try_new(1, vec![map(-6., -3.), map(6., -3.)], vec![0., 0., 12., 12.])
                .unwrap();
        let constrained = view
            .edge_point_cursor(&edge, None, pointer, area(), &doc, modes)
            .unwrap();
        assert!((constrained.parameter - 2.3).abs() < 1e-5, "{kind:?}");
    }
}

#[test]
fn real_pointer_near_pick_keeps_source_height_without_selecting_geometry() {
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
                                osnap: ObjectSnapModes::only(ObjectSnapKind::Near),
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
        .unwrap()
        + Vec2::new(0., 2.);
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
            .distance_to(p(-4., 0., 7.))
            .unwrap()
            < 1e-10
    );
    assert!(output.selection_click.is_none() && output.edge_click.is_none());
}
