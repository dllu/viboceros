use super::*;

fn grid(n: usize, m: usize) -> Vec<Point3> {
    (0..m)
        .flat_map(|j| {
            (0..n).map(move |i| {
                Point3::try_new(i as Real, j as Real, (i * j) as Real * 0.125).unwrap()
            })
        })
        .collect()
}

#[test]
fn open_grids_interpolate_mean_chord_stations_for_every_degree() {
    let points = grid(13, 7);
    for degree in 0..=13 {
        let s = NurbsSurface::try_through_point_grid(&points, [13, 7], [degree; 2], [false; 2])
            .unwrap();
        assert_eq!(s.degree_u(), degree.clamp(1, 11));
        assert_eq!(s.degree_v(), degree.clamp(1, 6));
        let u = basis::Direction::new(&points, [13, 7], 0, s.degree_u(), false).unwrap();
        let v = basis::Direction::new(&points, [13, 7], 1, s.degree_v(), false).unwrap();
        for (j, &tv) in v.parameters.iter().enumerate() {
            for (i, &tu) in u.parameters.iter().enumerate() {
                assert!(
                    s.evaluate(tu, tv)
                        .unwrap()
                        .distance_to(points[j * 13 + i])
                        .unwrap()
                        < 1e-11
                );
            }
        }
    }
}

#[test]
fn periodic_constraints_use_boundary_span_continuation_and_repeat_controls_exactly() {
    let points = grid(7, 6);
    for degree in 2..=5 {
        let s =
            NurbsSurface::try_through_point_grid(&points, [7, 6], [degree; 2], [true; 2]).unwrap();
        assert!(s.is_periodic_u() && s.is_periodic_v());
        let u = basis::Direction::new(&points, [7, 6], 0, degree, true).unwrap();
        let v = basis::Direction::new(&points, [7, 6], 1, degree, true).unwrap();
        assert!(u.parameters[0] < *s.domain_u().start());
        assert!(v.parameters[0] < *s.domain_v().start());
        for (j, &tv) in v.parameters.iter().enumerate() {
            for (i, &tu) in u.parameters.iter().enumerate() {
                assert!(
                    s.evaluate_extended(tu, tv)
                        .unwrap()
                        .distance_to(points[j * 7 + i])
                        .unwrap()
                        < 1e-11
                );
            }
        }
        assert_eq!(s.control_points()[0], s.control_points()[7]);
        assert_eq!(s.control_points()[0], s.control_points()[6 * (7 + degree)]);
    }
}

#[test]
fn control_grids_retain_locations_order_and_unit_knot_spacing() {
    let points = grid(5, 6);
    let s = NurbsSurface::try_control_point_grid(&points, [5, 6], [3, 2]).unwrap();
    assert_eq!(s.domain_u(), 0.0..=2.0);
    assert_eq!(s.domain_v(), 0.0..=4.0);
    assert_eq!(
        s.control_points()
            .iter()
            .map(|p| p.point())
            .collect::<Vec<_>>(),
        points
    );
    assert!(s.control_points().iter().all(|p| p.weight() == 1.0));
}

#[test]
fn malformed_and_degenerate_grids_fail_without_allocation_overflow() {
    let points = grid(3, 3);
    for count in [[0, 0], [1, 9], [3, 4], [usize::MAX, 2]] {
        assert!(NurbsSurface::try_control_point_grid(&points, count, [3; 2]).is_err());
        assert!(NurbsSurface::try_through_point_grid(&points, count, [3; 2], [false; 2]).is_err());
    }
    assert!(
        NurbsSurface::try_through_point_grid(&vec![points[0]; 9], [3, 3], [3; 2], [false; 2])
            .is_err()
    );
    assert!(
        NurbsSurface::try_through_point_grid(&grid(2, 3), [2, 3], [1; 2], [true, false]).is_err()
    );
}

#[test]
fn basis_continuation_retains_negative_bernstein_coefficients() {
    for t in [-2.0, -0.125, 0.0, 0.2, 1.0, 1.25, 3.0] {
        let b = crate::nurbs::bspline_basis_values_extended(
            &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            3,
            4,
            t,
        )
        .unwrap();
        let q: Real = 1.0 - t;
        let expected = [q * q * q, 3.0 * q * q * t, 3.0 * q * t * t, t * t * t];
        for (a, b) in b.into_iter().zip(expected) {
            assert!((a - b).abs() < 1e-13);
        }
    }
}

#[test]
fn bilinear_grid_reproduction_is_independent_of_interpolation_degree() {
    let points = grid(13, 7);
    for degree in [1, 2, 3, 5, 11] {
        let s = NurbsSurface::try_through_point_grid(&points, [13, 7], [degree; 2], [false; 2])
            .unwrap();
        for i in 0..=16 {
            for j in 0..=17 {
                let (a, b) = (i as Real / 16.0, j as Real / 17.0);
                let expected = Point3::try_new(12.0 * a, 6.0 * b, 9.0 * a * b).unwrap();
                assert!(
                    s.evaluate(s.parameter_at_u(a).unwrap(), s.parameter_at_v(b).unwrap())
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-11
                );
            }
        }
    }
}

#[test]
fn point_grid_solves_preserve_large_translations_and_extreme_uniform_scales() {
    let points = grid(6, 5);
    for closed in [[false; 2], [true; 2]] {
        let base = NurbsSurface::try_through_point_grid(&points, [6, 5], [3; 2], closed).unwrap();
        let offset = [2.0_f64.powi(40), -2.0_f64.powi(41), 2.0_f64.powi(42)];
        let shifted = points
            .iter()
            .map(|p| {
                Point3::try_from(std::array::from_fn(|i| p.to_array()[i] + offset[i])).unwrap()
            })
            .collect::<Vec<_>>();
        let translated =
            NurbsSurface::try_through_point_grid(&shifted, [6, 5], [3; 2], closed).unwrap();
        assert_eq!(translated.knots_u(), base.knots_u());
        assert_eq!(translated.knots_v(), base.knots_v());
        for (a, b) in translated
            .control_points()
            .iter()
            .zip(base.control_points())
        {
            assert_eq!(
                a.point().to_array(),
                std::array::from_fn(|i| b.point().to_array()[i] + offset[i])
            );
        }
        for scale in [1e-140, 1e140] {
            let scaled = points
                .iter()
                .map(|p| Point3::try_from(p.to_array().map(|x| x * scale)).unwrap())
                .collect::<Vec<_>>();
            let s = NurbsSurface::try_through_point_grid(&scaled, [6, 5], [3; 2], closed).unwrap();
            for (a, b) in s.control_points().iter().zip(base.control_points()) {
                for (a, b) in a.point().to_array().into_iter().zip(b.point().to_array()) {
                    assert!((a / scale - b).abs() < 1e-11);
                }
            }
        }
    }
}
