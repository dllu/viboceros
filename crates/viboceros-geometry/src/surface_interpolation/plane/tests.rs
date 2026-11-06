use super::*;
fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
#[test]
fn infinite_plane_interpolation_preserves_projected_stations_and_the_complete_plane() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    for normal in [[0., 0., 1.], [1., 2., 3.]] {
        let plane = Plane::new(
            point([1e6, -2e6, 3e6]),
            Vector3::try_from(normal)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
        );
        let frame = Frame3::try_from_unit_normal(plane.origin(), plane.normal()).unwrap();
        let coordinates = [
            [-4., -3., 5.],
            [-2., 2., -1.],
            [0., -1.5, 3.],
            [2., 3., 7.],
            [4., -2., 2.],
        ];
        let points = coordinates.map(|p| frame.point_at(p).unwrap());
        for closed in [false, true] {
            let result = try_interpolate_curve_on_plane(plane, &points, closed, tolerance).unwrap();
            let mut t = *result.curve.domain().start();
            for i in 0..points.len() {
                if i > 0 {
                    t += (coordinates[i][0] - coordinates[i - 1][0])
                        .hypot(coordinates[i][1] - coordinates[i - 1][1]);
                }
                let expected = frame
                    .point_at([coordinates[i][0], coordinates[i][1], 0.])
                    .unwrap();
                assert!(
                    expected
                        .distance_to(
                            result
                                .curve
                                .evaluate(t.min(*result.curve.domain().end()))
                                .unwrap()
                        )
                        .unwrap()
                        < 1e-6
                );
            }
            assert!(result.surface_image_bound <= 5e-7);
            assert_eq!(result.curve.is_closed().unwrap(), closed);
            for control in result.curve.control_points() {
                assert!(
                    plane
                        .directional_projection_within_tolerance(
                            control.point(),
                            control.point(),
                            plane.normal().as_vector(),
                            5e-7
                        )
                        .unwrap()
                );
            }
        }
    }
}
#[test]
fn collapsed_projected_inputs_and_unbounded_input_counts_decline() {
    let plane = Plane::new(
        point([0.; 3]),
        Vector3::try_from([0., 0., 1.])
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
    );
    assert!(
        try_interpolate_curve_on_plane(
            plane,
            &[point([1., 2., 3.]), point([1., 2., 7.])],
            false,
            Tolerance::DEFAULT
        )
        .is_err()
    );
    assert!(
        try_interpolate_curve_on_plane(
            plane,
            &vec![point([0.; 3]); 257],
            false,
            Tolerance::DEFAULT
        )
        .is_err()
    );
}
