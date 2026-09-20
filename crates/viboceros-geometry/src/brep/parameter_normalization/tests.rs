use super::*;

#[test]
fn uv_topology_normalization_keeps_independent_axis_ranges_and_origins() {
    let tiny = Real::from_bits(1);
    for [u, v] in [
        [[0., 1e-200], [0.4, 1.4]],
        [[0.4, 1.4], [0., 1e-200]],
        [[-Real::MAX, Real::MAX], [0., tiny]],
        [[0., tiny], [-Real::MAX, Real::MAX]],
        [[1e300, 1e300_f64.next_up()], [-tiny, tiny]],
        [[-tiny, tiny], [-1e300, (-1e300_f64).next_up()]],
    ] {
        let corners = [[u[0], v[0]], [u[1], v[0]], [u[1], v[1]], [u[0], v[1]]];
        let unit = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
        for first in 0..4 {
            let points = (0..4)
                .map(|i| Point2::try_from(corners[(first + i) % 4]).unwrap())
                .collect::<Vec<_>>();
            let normalization = TrimParameterNormalization::try_from_points(&points)
                .unwrap()
                .unwrap();
            for (i, point) in points.into_iter().enumerate() {
                let index = (first + i) % 4;
                assert_eq!(
                    normalization.normalize(point).unwrap(),
                    [
                        unit[index][0] - unit[first][0],
                        unit[index][1] - unit[first][1]
                    ]
                );
            }
        }
    }
}

#[test]
fn trim_parameter_uncertainty_scales_with_width_not_origin_or_unit_floor() {
    let tolerance = Tolerance::DEFAULT;
    for (domain, scale) in [
        ([2e-200, 6e-200], 1e-200),
        ([2e200, 6e200], 1e200),
        ([1e12 + 2., 1e12 + 6.], 1.),
    ] {
        for (actual, expected) in [
            (
                trim_parameter_epsilon(domain, tolerance),
                trim_parameter_epsilon([2., 6.], tolerance) * scale,
            ),
            (
                floating_parameter_epsilon(domain),
                floating_parameter_epsilon([2., 6.]) * scale,
            ),
        ] {
            assert!((actual / expected - 1.).abs() < 8. * Real::EPSILON);
        }
    }
    let tiny = Real::from_bits(1);
    assert_eq!(trim_parameter_epsilon([0., tiny], tolerance), 0.);
    assert_eq!(floating_parameter_epsilon([0., tiny]), 0.);
    let large = trim_parameter_epsilon([-Real::MAX, Real::MAX], tolerance);
    assert!(large.is_finite() && large > 0.);
    assert!((large / (Real::MAX * 2e-12) - 1.).abs() < 4. * Real::EPSILON);
}

#[test]
fn independent_axis_scaling_does_not_fabricate_area_for_constant_or_collinear_data() {
    let p = |x, y| Point2::try_new(x, y).unwrap();
    assert!(
        TrimParameterNormalization::try_from_points(&[p(0.3, 0.4); 3])
            .unwrap()
            .is_none()
    );
    for points in [
        [p(0., 0.4), p(1e-200, 0.4), p(2e-200, 0.4)],
        [p(0., 0.), p(1., 1e-200), p(2., 2e-200)],
    ] {
        let n = TrimParameterNormalization::try_from_points(&points)
            .unwrap()
            .unwrap();
        let normalized = points.map(|p| n.normalize(p).unwrap());
        assert_eq!(
            polygon_cross(normalized[0], normalized[1], normalized[2]),
            0.
        );
    }
}
