use super::*;

fn p(a: [f64; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}
fn v(a: [f64; 3]) -> Vector3 {
    Vector3::try_from(a).unwrap()
}

#[test]
fn supporting_line_is_unclamped_and_exactly_defined_by_its_endpoints() {
    let line = PointProjection3::onto_line(p([1., 2., 3.]), p([5., 7., 11.])).unwrap();
    assert_eq!(
        line.project(p([0.; 3])).unwrap(),
        p([-47. / 105., 20. / 105., 11. / 105.])
    );
    assert_eq!(line.project(p([1., 2., 3.])).unwrap(), p([1., 2., 3.]));
    assert_eq!(line.project(p([9., 12., 19.])).unwrap(), p([9., 12., 19.]));
    let reverse = PointProjection3::onto_line(p([5., 7., 11.]), p([1., 2., 3.])).unwrap();
    assert_eq!(
        line.project(p([7., -3., 9.])),
        reverse.project(p([7., -3., 9.]))
    );
}

#[test]
fn plane_variants_agree_without_assuming_a_unit_normal() {
    let points = [p([1., 2., 3.]), p([5., 7., 11.]), p([-2., 3., 5.])];
    let plane = PointProjection3::onto_three_point_plane(points).unwrap();
    for scale in [1., -1., 2_f64.powi(900), 2_f64.powi(-900)] {
        let normal =
            PointProjection3::onto_plane(points[0], v([2. * scale, -32. * scale, 19. * scale]))
                .unwrap();
        for point in [
            p([0.; 3]),
            p([7., -3., 9.]),
            points[0],
            points[1],
            points[2],
        ] {
            assert_eq!(plane.project(point), normal.project(point));
        }
    }
    let parallel =
        PointProjection3::onto_plane_parallel_to(points[0], points[1], v([0., 0., 1.])).unwrap();
    assert_eq!(
        parallel.project(p([0.; 3])).unwrap(),
        p([-15. / 41., 12. / 41., 0.])
    );
}

#[test]
fn coordinate_projections_preserve_extreme_tangential_coordinates() {
    for axis in 0..3 {
        let mut normal = [0.; 3];
        normal[axis] = f64::from_bits(1);
        let origin = p([1., 2., 3.]);
        let mut end = origin.to_array();
        end[axis] = f64::MAX;
        let line = PointProjection3::onto_line(origin, p(end)).unwrap();
        let plane = PointProjection3::onto_plane(origin, v(normal)).unwrap();
        let input = [-f64::MAX, f64::from_bits(1), f64::MAX];
        assert_eq!(
            line.project(p(input)).unwrap().to_array(),
            std::array::from_fn(|i| if i == axis {
                input[i]
            } else {
                origin.to_array()[i]
            })
        );
        assert_eq!(
            plane.project(p(input)).unwrap().to_array(),
            std::array::from_fn(|i| if i == axis {
                origin.to_array()[i]
            } else {
                input[i]
            })
        );
    }
}

#[test]
fn exact_displacements_survive_overflow_and_anisotropic_underflow() {
    let huge = 2_f64.powi(1023);
    let tiny = f64::from_bits(1);
    let line = PointProjection3::onto_line(p([-huge, -huge, 2.]), p([huge, huge, 2.])).unwrap();
    assert_eq!(line.project(p([7., -7., 9.])).unwrap(), p([0., 0., 2.]));
    let line = PointProjection3::onto_line(p([0.; 3]), p([huge, tiny, 0.])).unwrap();
    assert_eq!(
        line.project(p([huge, 0., 4.])).unwrap(),
        p([huge, tiny, 0.])
    );
    let plane = PointProjection3::onto_three_point_plane([
        p([0.; 3]),
        p([huge, tiny, 0.]),
        p([-huge, tiny, 0.]),
    ])
    .unwrap();
    assert_eq!(
        plane.project(p([1., tiny, huge])).unwrap(),
        p([1., tiny, 0.])
    );
}

#[test]
fn cancellation_preserves_small_plane_origins() {
    for exponent in [54, 100, 500, 1023] {
        let huge = 2_f64.powi(exponent);
        for offset in [1., -1., 2_f64.powi(-500)] {
            let plane = PointProjection3::onto_plane(p([offset, 0., 0.]), v([1., 1., 0.])).unwrap();
            assert_eq!(
                plane.project(p([huge, huge, 7.])).unwrap(),
                p([offset / 2., offset / 2., 7.])
            );
        }
    }
}

#[test]
fn only_degenerate_definitions_and_unrepresentable_results_are_rejected() {
    let zero = p([0.; 3]);
    assert!(PointProjection3::onto_line(zero, zero).is_err());
    assert!(PointProjection3::onto_plane(zero, v([0.; 3])).is_err());
    assert!(
        PointProjection3::onto_three_point_plane([zero, p([1., 2., 3.]), p([2., 4., 6.])]).is_err()
    );
    assert!(
        PointProjection3::onto_plane_parallel_to(zero, p([1., 2., 3.]), v([2., 4., 6.])).is_err()
    );
    let plane = PointProjection3::onto_plane(p([f64::MAX, f64::MAX, 0.]), v([1., 1., 0.])).unwrap();
    assert!(plane.project(p([f64::MAX, -f64::MAX, 0.])).is_err());
    assert_eq!(
        plane.project(p([0.; 3])).unwrap(),
        p([f64::MAX, f64::MAX, 0.])
    );
}
