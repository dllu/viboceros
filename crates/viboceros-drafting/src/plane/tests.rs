use super::*;

#[test]
fn ortho_uses_plane_axes_and_preserves_the_anchor_elevation() {
    let plane = frame();
    let anchor = plane.point_at([2.0, -3.0, 7.0]).unwrap();
    let local = plane.with_origin(anchor);
    for (cursor, angle, expected) in [
        ([3.0, 1.0, 0.0], 90.0, [3.0, 0.0, 0.0]),
        ([1.0, -4.0, 0.0], 90.0, [0.0, -4.0, 0.0]),
        ([3.0, 2.0, 0.0], 45.0, [2.5, 2.5, 0.0]),
        ([-3.0, 2.0, 0.0], 180.0, [-3.0, 0.0, 0.0]),
    ] {
        let point = ortho_point(local.point_at(cursor).unwrap(), anchor, plane, angle).unwrap();
        let actual = local.coordinates_of(point).unwrap();
        for index in 0..3 {
            assert!((actual[index] - expected[index]).abs() < 1e-12);
        }
    }
    assert_eq!(ortho_point(anchor, anchor, plane, 90.0), Some(anchor));
    let fine_cursor = local.point_at([3.0, 2.0, 0.0]).unwrap();
    let fine_result = ortho_point(fine_cursor, anchor, plane, f64::MIN_POSITIVE).unwrap();
    for (actual, expected) in fine_result
        .to_array()
        .into_iter()
        .zip(fine_cursor.to_array())
    {
        assert!((actual - expected).abs() < 1e-12);
    }
    for angle in [0.0, -1.0, 181.0, f64::NAN, f64::INFINITY] {
        assert_eq!(ortho_point(anchor, anchor, plane, angle), None);
    }
    let world = Frame3::try_from_directions(
        point(0.0, 0.0, 0.0),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let huge = point(f64::MAX * 0.8, f64::MAX * 0.8, 0.0);
    let projected = ortho_point(huge, point(0.0, 0.0, 0.0), world, 45.0).unwrap();
    assert!((projected.x() - huge.x()).abs() <= 4.0 * f64::EPSILON * huge.x());
    assert!((projected.y() - huge.y()).abs() <= 4.0 * f64::EPSILON * huge.y());
}

#[test]
fn cplane_z_tracking_inverts_parallel_and_perspective_projection() {
    let plane = Frame3::try_from_directions(
        point(0.0, 0.0, 0.0),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let anchor = point(1.0, 2.0, 0.0);
    let parallel = |candidate: Point3| Some([10.0 * candidate.z(), 0.0, 1.0]);
    let (candidate, distance) = ortho_z_projected(anchor, plane, [30.0, 4.0], parallel).unwrap();
    assert_eq!(candidate, point(1.0, 2.0, 3.0));
    assert_eq!(distance, 4.0);

    let perspective = |candidate: Point3| {
        let depth = 10.0 + candidate.z();
        (depth > 0.0).then_some([100.0 * candidate.z() / depth, 0.0, depth])
    };
    let target = perspective(point(1.0, 2.0, 3.0)).unwrap();
    let (candidate, distance) =
        ortho_z_projected(anchor, plane, [target[0], 4.0], perspective).unwrap();
    assert!((candidate.z() - 3.0).abs() < 1e-12);
    assert!((distance - 4.0).abs() < 1e-12);
    assert_eq!(candidate.x(), 1.0);
    assert_eq!(candidate.y(), 2.0);
    let one_sided =
        |candidate: Point3| (candidate.z() <= 0.0).then_some([10.0 * candidate.z(), 0.0, 1.0]);
    let (candidate, distance) = ortho_z_projected(anchor, plane, [-20.0, 0.0], one_sided).unwrap();
    assert_eq!(candidate.z(), -2.0);
    assert_eq!(distance, 0.0);
    assert!(ortho_z_projected(anchor, plane, [1.0, 2.0], |_| Some([0.0, 0.0, 1.0])).is_none());
}

#[test]
fn projected_anchor_capture_does_not_require_unrepresentable_axis_candidates() {
    let plane = frame();
    let anchor = point(f64::MAX, 0.0, 7.0);
    let cursor = point(-f64::MAX, 0.0, 0.0);
    let snap = orthogonal_track_projected(cursor, anchor, plane, [10.0, 20.0], 1.0, |candidate| {
        (candidate == anchor).then_some([10.0, 20.0])
    })
    .unwrap()
    .unwrap();
    assert_eq!(snap.axis(), TrackAxis::Both);
    assert_eq!(snap.point(), anchor);
}

#[test]
fn fine_grid_spacing_does_not_overflow_a_finite_coordinate() {
    let plane = Frame3::try_from_directions(
        point(0.0, 0.0, 0.0),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for spacing in [f64::from_bits(1), f64::MIN_POSITIVE, 0.25] {
        for x in [1.0, 1e200, f64::MAX, -f64::MAX] {
            let p = point(x, 0.0, 3.0);
            assert_eq!(snap_to_grid(p, plane, spacing).unwrap(), p);
        }
    }
    for spacing in [0.5, 1.0, 2.0, 8.0] {
        for index in -65..=65 {
            let x = f64::from(index) / 4.0;
            let expected = (x / spacing).round() * spacing;
            assert_eq!(
                snap_to_grid(point(x, -x, 3.0), plane, spacing).unwrap(),
                point(expected, -expected, 3.0)
            );
        }
    }
    for sign in [-1.0, 1.0] {
        assert_eq!(
            snap_to_grid(
                point(sign * f64::from_bits(5), 0.0, 0.0),
                plane,
                f64::from_bits(3)
            )
            .unwrap(),
            point(sign * f64::from_bits(6), 0.0, 0.0)
        );
        // The nearest grid point genuinely exceeds the finite model range.
        assert!(snap_to_grid(point(sign * f64::MAX, 0.0, 0.0), plane, 1e308).is_err());
    }
}
use viboceros_geometry::Tolerance;

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_directions(
        point(3., -4., 5.),
        Vector3::try_new(1., 2., 3.).unwrap(),
        Vector3::try_new(-4., 8., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn oblique_line_intersections_and_plane_grid_round_trip_local_coordinates() {
    let plane = frame();
    for xy in [[1.49, -1.51, 0.], [-1.49, 1.51, 0.], [30.2, -41.3, 0.]] {
        let expected = plane.point_at(xy).unwrap();
        let start = plane.point_at([xy[0], xy[1], 7.]).unwrap();
        let normal = plane.z_axis().as_vector();
        let result = intersect_view_line(start, normal.scaled(-1.).unwrap(), plane, true)
            .unwrap()
            .unwrap();
        assert!(result.distance_to(expected).unwrap() < 1e-12);
        assert!(
            intersect_view_line(start, normal, plane, true)
                .unwrap()
                .is_none()
        );
        assert!(
            intersect_view_line(start, plane.x_axis().as_vector(), plane, false)
                .unwrap()
                .is_none()
        );
        let elevated = plane.point_at([xy[0], xy[1], 3.25]).unwrap();
        let snapped = snap_to_grid(elevated, plane, 1.).unwrap();
        assert!(
            snapped
                .distance_to(
                    plane
                        .point_at([xy[0].round(), xy[1].round(), 3.25])
                        .unwrap()
                )
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn projected_tracking_obeys_pixel_capture_in_arbitrary_planes() {
    let plane = frame();
    let anchor = plane.point_at([2., 3., 4.]).unwrap();
    let project = |point| {
        let [x, y, _] = plane.coordinates_of(point).ok()?;
        Some([x * 40. + y * 9., y * 30.])
    };
    for (offset, axis) in [
        ([4., 0.1, 0.], TrackAxis::Horizontal),
        ([0.1, 4., 0.], TrackAxis::Vertical),
        ([0.05, 0.05, 0.], TrackAxis::Both),
    ] {
        let cursor = plane.with_origin(anchor).point_at(offset).unwrap();
        let pointer = project(cursor).unwrap();
        let track = orthogonal_track_projected(cursor, anchor, plane, pointer, 8., project)
            .unwrap()
            .unwrap();
        assert_eq!(track.axis(), axis);
        let pixel = project(track.point()).unwrap();
        assert!((pixel[0] - pointer[0]).hypot(pixel[1] - pointer[1]) <= 8.);
        assert!(
            plane
                .with_origin(anchor)
                .coordinates_of(track.point())
                .unwrap()[2]
                .abs()
                < 1e-12
        );
    }
    let cursor = plane.with_origin(anchor).point_at([2., 2., 0.]).unwrap();
    assert!(
        orthogonal_track_projected(cursor, anchor, plane, project(cursor).unwrap(), 8., project)
            .unwrap()
            .is_none()
    );
}

#[test]
fn invalid_spacing_and_unrepresentable_plane_offsets_return_errors_not_panics() {
    let plane = frame();
    for spacing in [0., -1., f64::INFINITY, f64::NAN] {
        assert!(snap_to_grid(point(1., 2., 3.), plane, spacing).is_err());
    }
    let far = plane.with_origin(point(f64::MAX, 0., 0.));
    assert!(snap_to_grid(point(-f64::MAX, 0., 0.), far, 1.).is_err());
    assert!(
        intersect_view_line(
            point(1., 2., 3.),
            Vector3::try_new(0., 0., 0.).unwrap(),
            plane,
            true
        )
        .is_err()
    );
}
