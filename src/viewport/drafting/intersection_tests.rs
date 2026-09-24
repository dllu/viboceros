use super::*;
use viboceros_drafting::ObjectSnapKind;
use viboceros_geometry::LineSegment;

#[test]
fn straight_intersections_reach_parallel_and_perspective_point_prompts() {
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Perspective,
    ] {
        let point = |x, y| match kind {
            ViewKind::Front => Point3::try_new(x, 7., y).unwrap(),
            ViewKind::Right => Point3::try_new(7., x, y).unwrap(),
            _ => Point3::try_new(x, y, 7.).unwrap(),
        };
        let mut document = Document::default();
        for (a, b) in [
            (point(-2., 0.), point(2., 0.)),
            (point(0., -2.), point(0., 2.)),
        ] {
            document
                .add_geometry(Geometry::Line(
                    LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap(),
                ))
                .unwrap();
        }
        let mut view = Viewport::new(kind);
        view.target = NaVector3::from(point(0., 0.).to_array());
        let pointer = view.project(point(0., 0.), area).unwrap() + Vec2::new(2., 2.);
        let cursor = view
            .drafting_cursor(
                pointer,
                area,
                &document,
                DraftingInput {
                    active: true,
                    osnap: ObjectSnapModes::only(ObjectSnapKind::Intersection),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            cursor.object_snap.unwrap().kind(),
            ObjectSnapKind::Intersection,
            "{kind:?}"
        );
        assert!(
            cursor.point.distance_to(point(0., 0.)).unwrap() < 1e-9,
            "{kind:?}"
        );
    }
}
