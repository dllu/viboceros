use super::*;

#[test]
fn both_mesh_endpoints_inside_select_the_nearer_endpoint_not_curve_near() {
    use super::super::ProjectedSnapMetric;
    for scale in [1e-200, 1., 1e200] {
        let p = |x, y| Point3::try_new(x * scale, y * scale, 1.).unwrap();
        let a = p(-0.75, 0.5);
        let b = p(0.5, 0.5);
        let metric = ProjectedSnapMetric {
            cursor: [0.; 2],
            capture_radius: scale,
            project: |p: Point3| Some([p.x(), p.y()]),
        };
        for (a, b) in [(a, b), (b, a)] {
            let Capture::Point(mesh) = capture_mesh(a, b, &metric) else {
                panic!()
            };
            assert_eq!(mesh, p(0.5, 0.5));
            let Capture::Point(curve) = capture(a, b, &metric) else {
                panic!()
            };
            assert!((curve.x() / scale).abs() < 1e-14);
            assert_eq!(curve.y(), 0.5 * scale);
        }
    }
}

#[test]
fn inside_mesh_endpoint_ties_retain_wire_orientation_even_at_different_depths() {
    use super::super::ProjectedSnapMetric;
    let p = |x, z| Point3::try_new(x, 0.5, z).unwrap();
    let metric = ProjectedSnapMetric {
        cursor: [0.; 2],
        capture_radius: 1.,
        project: |p: Point3| Some([p.x(), p.y()]),
    };
    for (a, b) in [
        (p(-0.5, 1.), p(0.5, 9.)),
        (p(0.5, 9.), p(-0.5, 1.)),
        (p(0.5, 1.), p(0.5, 9.)),
        (p(0.5, 9.), p(0.5, 1.)),
    ] {
        let Capture::Point(point) = capture_mesh(a, b, &metric) else {
            panic!()
        };
        assert_eq!(point, a);
    }
}

#[test]
fn clipping_does_not_turn_an_interior_point_into_a_mesh_endpoint() {
    use super::super::ProjectedSnapMetric;
    let a = Point3::try_new(-2., 0.5, 0.).unwrap();
    let b = Point3::try_new(0.5, 0.5, 1.).unwrap();
    let metric = ProjectedSnapMetric {
        cursor: [0.; 2],
        capture_radius: 1.,
        project: |p: Point3| (p.z() >= 0.5).then_some([p.x(), p.y()]),
    };
    for (a, b) in [(a, b), (b, a)] {
        let Capture::Point(point) = capture_mesh(a, b, &metric) else {
            panic!()
        };
        assert!(
            point
                .distance_to(Point3::try_new(0., 0.5, 0.8).unwrap())
                .unwrap()
                < 1e-14
        );
    }
}

#[test]
fn one_or_no_inside_endpoint_still_uses_the_interior_wire_target() {
    use super::super::ProjectedSnapMetric;
    let p = |x| Point3::try_new(x, 0.5, 1.).unwrap();
    let metric = ProjectedSnapMetric {
        cursor: [0.; 2],
        capture_radius: 1.,
        project: |p: Point3| Some([p.x(), p.y()]),
    };
    for (a, b) in [
        (p(-0.5), p(2.)),
        (p(2.), p(-0.5)),
        (p(-2.), p(2.)),
        (p(2.), p(-2.)),
    ] {
        let Capture::Point(point) = capture_mesh(a, b, &metric) else {
            panic!()
        };
        assert!(point.distance_to(p(0.)).unwrap() < 1e-14);
    }
}

#[test]
fn mesh_depth_weighting_matches_independent_fraction_reference() {
    use super::super::ProjectedSnapMetric;
    let mut count = 0;
    for (index, row) in include_str!("mesh_reference.csv")
        .lines()
        .enumerate()
        .skip(1)
    {
        let v: Vec<Real> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 25);
        let p = |i| Point3::try_new(v[i], v[i + 1], v[i + 2]).unwrap();
        let metric = ProjectedSnapMetric {
            cursor: [v[18], v[19]],
            capture_radius: v[20],
            project: |point: Point3| {
                let xyz = point.to_array();
                let h: [Real; 3] = std::array::from_fn(|i| {
                    (0..3).fold(v[4 * i + 3], |sum, j| v[4 * i + j].mul_add(xyz[j], sum))
                });
                (h[2] > 0.).then_some([h[0] / h[2], h[1] / h[2]])
            },
        };
        let Capture::Point(actual) = capture_mesh(p(12), p(15), &metric) else {
            panic!("mesh reference row {index}: missing capture");
        };
        for (a, e) in actual.to_array().into_iter().zip(p(21).to_array()) {
            assert!(
                (a - e).abs() <= 3e-11 * e.abs().max(1.),
                "mesh reference row {index}: {actual:?} != {:?}",
                p(21)
            );
        }
        assert!(
            (metric.distance(actual).unwrap() - v[24].sqrt()).abs() <= 3e-11,
            "mesh reference row {index}: wrong distance"
        );
        count += 1;
    }
    assert_eq!(count, 630);
}

#[test]
fn clipped_projective_lines_match_an_independent_exact_rational_corpus() {
    use super::super::ProjectedSnapMetric;
    let mut count = 0;
    for (index, row) in include_str!("reference.csv").lines().enumerate().skip(1) {
        let values: Vec<Real> = row.split(',').map(|v| v.parse().unwrap()).collect();
        assert_eq!(values.len(), 25);
        let point = |i| Point3::try_new(values[i], values[i + 1], values[i + 2]).unwrap();
        let expected = point(21);
        let expected_distance = values[24].sqrt();
        let metric = ProjectedSnapMetric {
            cursor: [values[18], values[19]],
            capture_radius: expected_distance + 1.,
            project: |p: Point3| {
                let xyz = p.to_array();
                let h: [Real; 3] = std::array::from_fn(|i| {
                    (0..3).fold(values[4 * i + 3], |sum, j| {
                        values[4 * i + j].mul_add(xyz[j], sum)
                    })
                });
                (h[2] >= values[20]).then_some([h[0] / h[2], h[1] / h[2]])
            },
        };
        let Capture::Point(actual) = capture(point(12), point(15), &metric) else {
            panic!("reference row {index}: missing capture");
        };
        for (a, e) in actual.to_array().into_iter().zip(expected.to_array()) {
            assert!(
                (a - e).abs() <= 3e-11 * e.abs().max(1.),
                "reference row {index}: {actual:?} != {expected:?}"
            );
        }
        let distance = metric
            .distance(actual)
            .expect("captured point must be visible");
        assert!(
            (distance - expected_distance).abs() <= 3e-11 * expected_distance.max(1.),
            "reference row {index}: distance {distance} != {expected_distance}"
        );
        let hover = super::distance(point(12), point(15), &metric).unwrap();
        assert!(
            (hover - expected_distance).abs() <= 3e-11 * expected_distance.max(1.),
            "reference row {index}: hover {hover} != {expected_distance}"
        );
        count += 1;
    }
    assert_eq!(count, 288);
}

#[test]
fn clipped_boundary_is_a_candidate_but_an_invisible_segment_is_not() {
    use super::super::ProjectedSnapMetric;
    let p = |x, z| Point3::try_new(x, 0., z).unwrap();
    let metric = ProjectedSnapMetric {
        cursor: [8., 0.],
        capture_radius: 10.,
        project: |p: Point3| (p.z() >= 0.5).then_some([p.x() / p.z(), p.y() / p.z()]),
    };
    for (a, b) in [(p(0., 1.), p(1., -1.)), (p(1., -1.), p(0., 1.))] {
        let Capture::Point(point) = capture(a, b, &metric) else {
            panic!()
        };
        assert_eq!(point, p(0.25, 0.5));
        assert_eq!(distance(a, b, &metric), Some(7.5));
    }
    assert!(matches!(
        capture(p(0., -1.), p(1., -2.), &metric),
        Capture::Unresolved
    ));
    assert_eq!(distance(p(0., -1.), p(1., -2.), &metric), None);
}

#[test]
fn asymmetric_screen_segment_does_not_round_an_interior_minimum_to_its_endpoint() {
    for far in [1e12, 1e100, 1e300] {
        let a = [-far, 0.125];
        let b = [1., 0.125];
        for (a, b) in [(a, b), (b, a)] {
            assert!((segment_distance(a, b).unwrap() - 0.125).abs() < 1e-12);
        }
    }
}

#[test]
fn projected_segment_distance_retains_tiny_and_overflowing_coordinate_scales() {
    for scale in [1e-300, 1., 1e300] {
        for (a, b, expected) in [
            ([3., -4.], [3., 4.], 3.),
            ([-3., 0.], [3., 0.], 0.),
            ([3., 4.], [6., 8.], 5.),
            ([3., 4.], [3., 4.], 5.),
        ] {
            let result = segment_distance(a.map(|v| v * scale), b.map(|v| v * scale)).unwrap();
            assert!((result / scale - expected).abs() < 1e-12);
            assert_eq!(
                result,
                segment_distance(b.map(|v| v * scale), a.map(|v| v * scale)).unwrap()
            );
        }
    }
    assert_eq!(
        segment_distance([1e308, -1e308], [1e308, 1e308]),
        Some(1e308)
    );
    assert_eq!(
        segment_distance([-Real::MAX, 0.], [Real::MAX, 0.]),
        Some(0.)
    );
    assert_eq!(segment_distance([0., 0.], [0., 0.]), Some(0.));
    assert_eq!(
        segment_distance([-1e300, 1e-300], [1e300, 1e-300]),
        Some(1e-300)
    );
}
