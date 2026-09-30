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
fn cplane_view_uses_the_visual_camera_target_and_screen_axes() {
    let mut top = Viewport::new(ViewKind::Top);
    top.target = NaVector3::new(3., 4., 5.);
    top.pan = Vec2::new(20., -40.);
    let top_camera = top.camera_snapshot();
    let aligned = top.construction_plane_aligned_to_view().unwrap();
    assert_eq!(aligned.origin(), point(2.5, 3., 5.));
    assert_eq!(aligned.axes(), WorldPlane::Top.frame().axes());
    assert_eq!(top.camera_snapshot(), top_camera);

    for (kind, preset) in [
        (ViewKind::Top, WorldPlane::Top),
        (ViewKind::Bottom, WorldPlane::Bottom),
        (ViewKind::Front, WorldPlane::Front),
        (ViewKind::Back, WorldPlane::Back),
        (ViewKind::Right, WorldPlane::Right),
        (ViewKind::Left, WorldPlane::Left),
    ] {
        let mut view = Viewport::new(kind);
        view.target = NaVector3::new(3., 4., 5.);
        let aligned = view.construction_plane_aligned_to_view().unwrap();
        assert_eq!(aligned.origin(), point(3., 4., 5.));
        assert_eq!(aligned.axes(), preset.frame().axes());
    }

    let mut plan = Viewport::new(ViewKind::Top);
    plan.plane.set(oblique_plane());
    plan.set_cplane_view(WorldPlane::Top);
    let camera_frame = plan.plan_frame;
    plan.plane.set(WorldPlane::Top.frame());
    let aligned = plan.construction_plane_aligned_to_view().unwrap();
    assert_eq!(aligned.origin(), camera_frame.origin());
    for (actual, expected) in aligned.axes().into_iter().zip(camera_frame.axes()) {
        for (actual, expected) in actual
            .as_vector()
            .to_array()
            .into_iter()
            .zip(expected.as_vector().to_array())
        {
            assert!((actual - expected).abs() < 1e-14);
        }
    }

    let mut perspective = Viewport::new(ViewKind::Perspective);
    perspective.target = NaVector3::new(3., 4., 5.);
    perspective.orbit_yaw = -std::f64::consts::FRAC_PI_3;
    let perspective_camera = perspective.camera_snapshot();
    let aligned = perspective.construction_plane_aligned_to_view().unwrap();
    assert_eq!(aligned.origin(), point(3., 4., 5.));
    let expected_x = [0.8660254037844387, 0.5, 0.];
    let expected_y = [-0.25, 0.43301270189221935, 0.8660254037844387];
    for (actual, expected) in aligned
        .x_axis()
        .as_vector()
        .to_array()
        .into_iter()
        .zip(expected_x)
    {
        assert!((actual - expected).abs() < 1e-14);
    }
    for (actual, expected) in aligned
        .y_axis()
        .as_vector()
        .to_array()
        .into_iter()
        .zip(expected_y)
    {
        assert!((actual - expected).abs() < 1e-14);
    }
    assert_eq!(perspective.camera_snapshot(), perspective_camera);
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
                osnap: viboceros_drafting::ObjectSnapModes::ALL,
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
fn planar_mode_uses_previous_pick_elevation_in_each_viewport() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for kind in [
        ViewKind::Top,
        ViewKind::Front,
        ViewKind::Right,
        ViewKind::Plan,
        ViewKind::Perspective,
    ] {
        let view = Viewport::new(kind);
        let plane = view.construction_plane();
        let anchor = plane.point_at([0.0, 0.0, 2.5]).unwrap();
        let target = plane.point_at([2.0, 1.5, 2.5]).unwrap();
        let pointer = view.project(target, rect).unwrap();
        for (planar, elevation) in [(false, 0.0), (true, 2.5)] {
            let cursor = view
                .drafting_cursor(
                    pointer,
                    rect,
                    &Document::default(),
                    DraftingInput {
                        active: true,
                        planar,
                        anchor: Some(anchor),
                        ..Default::default()
                    },
                )
                .unwrap();
            let coordinates = plane.coordinates_of(cursor.point).unwrap();
            assert!(
                (coordinates[2] - elevation).abs() < 1e-8,
                "{kind:?}, {planar}"
            );
            if planar {
                assert!(cursor.point.distance_to(target).unwrap() < 1e-5, "{kind:?}");
            }
        }
    }
    let view = Viewport::new(ViewKind::Top);
    let target = point(2.0, 1.5, 7.0);
    let mut document = Document::default();
    document.add_geometry(Geometry::Point(target)).unwrap();
    let cursor = view
        .drafting_cursor(
            view.project(target, rect).unwrap(),
            rect,
            &document,
            DraftingInput {
                active: true,
                planar: true,
                anchor: Some(point(0.0, 0.0, 2.5)),
                osnap: viboceros_drafting::ObjectSnapModes::ALL,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(cursor.point, target);
    assert!(cursor.object_snap.is_some());
}

#[test]
fn planar_mode_reinterprets_a_pick_from_another_viewport_in_the_current_cplane() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let top = Viewport::new(ViewKind::Top);
    let front = Viewport::new(ViewKind::Front);
    let previous = point(1.0, 7.0, 3.0);
    assert!(top.project(previous, rect).is_some());
    let plane = front.construction_plane();
    let elevation = plane.coordinates_of(previous).unwrap()[2];
    assert!(elevation.abs() > 1.0);
    let target = plane.point_at([2.0, 3.0, elevation]).unwrap();
    let pointer = front.project(target, rect).unwrap();
    for (enabled, expected) in [(false, 0.0), (true, elevation)] {
        let cursor = front
            .drafting_cursor(
                pointer,
                rect,
                &Document::default(),
                DraftingInput {
                    active: true,
                    planar: enabled,
                    anchor: Some(previous),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!((plane.coordinates_of(cursor.point).unwrap()[2] - expected).abs() < 1e-8);
    }
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
    view.set_world_view(ViewKind::Front).unwrap();
    assert_eq!(
        view.construction_plane(),
        WorldPlane::Front.frame().with_origin(plane.origin())
    );
    assert!(view.plane.undo());
    assert_eq!(view.construction_plane(), plane);
    assert_eq!(view.kind(), ViewKind::Front);
}
