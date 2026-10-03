use super::*;
use viboceros_command::translation::DestinationConstraint;
use viboceros_drafting::{PointFilter, PointFilterSession};

#[test]
fn vertical_mouse_picks_work_with_an_edge_on_cplane_and_keep_explicit_snap_sources() {
    let rect = Rect::from_min_size(Pos2::new(20., 30.), Vec2::new(800., 600.));
    let mut view = Viewport::new(ViewKind::Front);
    let plane = viboceros_command::construction_plane::WorldPlane::Top.frame();
    view.set_construction_plane(plane);
    let anchor = Point3::try_new(1., 2., 3.).unwrap();
    let aimed = Point3::try_new(4., 5., 9.).unwrap();
    let pointer = view.project(aimed, rect).unwrap();
    let mut document = Document::default();
    let input = DraftingInput {
        active: true,
        anchor: Some(anchor),
        ..Default::default()
    };
    let translation = DestinationConstraint {
        anchor,
        direction: Some(plane.z_axis()),
        distance: None,
    };
    assert!(
        view.filtered_drafting_cursor(pointer, rect, &document, input, None, None)
            .is_none()
    );
    let cursor = view
        .translation_drafting_cursor(
            pointer,
            rect,
            &document,
            input,
            None,
            None,
            Some(translation),
        )
        .unwrap();
    let expected = Point3::try_new(1., 2., 9.).unwrap();
    assert!(cursor.point.distance_to(expected).unwrap() < 1e-9);
    assert_eq!(cursor.source_point, cursor.point);

    document.add_geometry(Geometry::Point(aimed)).unwrap();
    let input = DraftingInput {
        osnap: ObjectSnapModes::ALL,
        ..input
    };
    let cursor = view
        .translation_drafting_cursor(
            pointer,
            rect,
            &document,
            input,
            None,
            None,
            Some(translation),
        )
        .unwrap();
    assert_eq!(cursor.source_point, aimed);
    assert_eq!(cursor.point, expected);
    assert!(cursor.object_snap.is_some());

    let filter = PointFilterSession::new(PointFilter::parse(".z").unwrap(), plane);
    let cursor = view
        .translation_drafting_cursor(
            pointer,
            rect,
            &document,
            input,
            Some(filter),
            None,
            Some(translation),
        )
        .unwrap();
    assert_eq!(cursor.source_point, aimed);
    assert_eq!(cursor.point, filter.preview_point(aimed).unwrap());
}

#[test]
fn translation_line_mouse_picks_keep_camera_local_precision_at_large_origins() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for kind in [ViewKind::Front, ViewKind::Perspective] {
        let mut view = Viewport::new(kind);
        let anchor = Point3::try_new(1e12, -1e12, 1e12).unwrap();
        view.target = NaVector3::from(anchor.to_array());
        let plane = viboceros_command::construction_plane::WorldPlane::Top
            .frame()
            .with_origin(anchor);
        view.set_construction_plane(plane);
        let expected = anchor
            .translated(plane.z_axis().as_vector().scaled(8.).unwrap())
            .unwrap();
        let pointer = view.project(expected, rect).unwrap();
        let cursor = view
            .translation_drafting_cursor(
                pointer,
                rect,
                &Document::default(),
                DraftingInput {
                    active: true,
                    anchor: Some(anchor),
                    ..Default::default()
                },
                None,
                None,
                Some(DestinationConstraint {
                    anchor,
                    direction: Some(plane.z_axis()),
                    distance: None,
                }),
            )
            .unwrap();
        assert!(
            cursor.point.distance_to(expected).unwrap() < 0.001,
            "{kind:?}: {:?}",
            cursor.point
        );
    }
}
