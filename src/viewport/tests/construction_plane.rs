use super::*;
use viboceros_geometry::Frame3;

#[test]
fn drafting_dash_tessellation_is_bounded_by_the_viewport() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let p = Pos2::new;
    for extend in [false, true] {
        assert_eq!(
            clip_drafting_line(p(-1000., 250.), p(1000., 250.), rect, extend),
            Some([p(0., 250.), p(800., 250.)])
        );
        assert_eq!(
            clip_drafting_line(p(-2., -3.), p(-2., 1.), rect, extend),
            None
        );
        assert_eq!(clip_drafting_line(p(2., 3.), p(2., 3.), rect, extend), None);
        assert_eq!(
            clip_drafting_line(p(f32::NAN, 3.), p(2., 3.), rect, extend),
            None
        );
        for (start, end, expected) in [
            (
                p(-f32::MAX, 250.),
                p(f32::MAX, 250.),
                [p(0., 250.), p(800., 250.)],
            ),
            (p(-1e30, -1e30), p(1e30, 1e30), [p(0., 0.), p(600., 600.)]),
            (p(1e30, 100.), p(0., 100.), [p(800., 100.), p(0., 100.)]),
        ] {
            assert_eq!(clip_drafting_line(start, end, rect, extend), Some(expected));
            assert_eq!(
                clip_drafting_line(end, start, rect, extend),
                Some([expected[1], expected[0]])
            );
            let swap = |point: Pos2| p(point.y, point.x);
            let swapped_rect = Rect::from_min_max(swap(rect.min), swap(rect.max));
            assert_eq!(
                clip_drafting_line(swap(start), swap(end), swapped_rect, extend),
                Some(expected.map(swap))
            );
            assert_eq!(
                clip_drafting_line(swap(end), swap(start), swapped_rect, extend),
                Some([swap(expected[1]), swap(expected[0])])
            );
        }
    }
    assert_eq!(
        clip_drafting_line(p(1., 2.), p(3., 4.), rect, false),
        Some([p(1., 2.), p(3., 4.)])
    );
    assert_eq!(
        clip_drafting_line(p(1., 2.), p(3., 4.), rect, true),
        Some([p(0., 1.), p(599., 600.)])
    );
}

fn oblique_plane() -> Frame3 {
    Frame3::try_from_directions(
        point(1., -2., 3.),
        Vector3::try_new(1., 2., 3.).unwrap(),
        Vector3::try_new(-4., 8., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn cplane_edits_do_not_move_camera_projection_depth_or_gpu_matrices() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Perspective,
    ] {
        let mut view = Viewport::new(kind);
        view.pan = Vec2::new(20., -30.);
        let p = point(2., 3., 4.);
        let before = view.project(p, rect);
        let depth = view.view_depth(p);
        let gpu = view.gpu_view_uniform(rect, None).view_projection;
        view.plane.set(oblique_plane());
        assert_eq!(view.project(p, rect), before);
        assert_eq!(view.view_depth(p), depth);
        assert_eq!(view.gpu_view_uniform(rect, None).view_projection, gpu);
        assert_eq!(view.kind(), kind);
    }
}

#[test]
fn each_camera_can_pick_on_an_oblique_plane_and_on_a_parallel_anchor_plane() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Perspective,
    ] {
        let mut view = Viewport::new(kind);
        let plane = oblique_plane();
        view.plane.set(plane);
        for elevation in [0., 2.5, -3.] {
            let expected = plane.point_at([1.25, -0.75, elevation]).unwrap();
            let pointer = view.project(expected, rect).unwrap();
            let anchor = if elevation == 0. {
                None
            } else {
                Some(plane.point_at([0., 0., elevation]).unwrap())
            };
            let actual = view
                .unproject_drafting_plane(pointer, rect, anchor)
                .unwrap();
            assert!(
                actual.distance_to(expected).unwrap() < 1e-5,
                "{kind:?}: {actual:?} != {expected:?}"
            );
        }
        let raw = plane.point_at([1.49, -1.51, 2.25]).unwrap();
        let snapped = view.snap_to_grid(raw).unwrap();
        assert!(
            snapped
                .distance_to(plane.point_at([1., -2., 2.25]).unwrap())
                .unwrap()
                < 1e-12
        );
        assert_eq!(
            view.grid_point(2., 3.).unwrap(),
            plane.point_at([2., 3., 0.]).unwrap()
        );
    }
}

#[test]
fn edge_on_cplane_has_no_free_pick_but_camera_space_object_snaps_remain_available() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let mut view = Viewport::new(ViewKind::Top);
    view.plane.set(WorldPlane::Front.frame());
    let mut document = Document::default();
    let expected = point(2., 3., 4.);
    document.add_geometry(Geometry::Point(expected)).unwrap();
    let pointer = view.project(expected, rect).unwrap();
    assert!(view.unproject_drafting_plane(pointer, rect, None).is_none());
    assert!(
        view.drafting_cursor(pointer, rect, &document, DraftingInput::default())
            .is_none()
    );
    let cursor = view
        .drafting_cursor(
            pointer,
            rect,
            &document,
            DraftingInput {
                active: true,
                osnap: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(cursor.point, expected);
    assert_eq!(
        cursor.object_snap.unwrap().kind(),
        viboceros_drafting::ObjectSnapKind::Point
    );
}

#[test]
fn smarttrack_uses_plane_axes_in_front_right_and_oblique_views() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Perspective,
    ] {
        for custom in [false, true] {
            let mut view = Viewport::new(kind);
            if custom {
                view.plane.set(oblique_plane());
            }
            let plane = view.construction_plane();
            let anchor = plane.point_at([1., 2., 0.]).unwrap();
            let expected = plane.point_at([4., 2., 0.]).unwrap();
            let target = plane.point_at([4., 2.02, 0.]).unwrap();
            let pointer = view.project(target, rect).unwrap();
            let cursor = view
                .drafting_cursor(
                    pointer,
                    rect,
                    &Document::default(),
                    DraftingInput {
                        active: true,
                        smart_track: true,
                        anchor: Some(anchor),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(
                cursor.track.unwrap().axis(),
                TrackAxis::Horizontal,
                "{kind:?}, {custom}"
            );
            assert!(cursor.point.distance_to(expected).unwrap() < 1e-5);
        }
    }
}

#[test]
fn view_presets_reset_the_plane_explicitly_and_plane_undo_retains_the_new_camera() {
    let mut view = Viewport::new(ViewKind::Top);
    let plane = oblique_plane();
    view.plane.set(plane);
    view.set_view_kind(ViewKind::Front);
    assert_eq!(view.construction_plane(), WorldPlane::Front.frame());
    assert!(view.plane.undo());
    assert_eq!(view.construction_plane(), plane);
    assert_eq!(view.kind(), ViewKind::Front);
}
