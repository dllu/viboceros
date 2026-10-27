use viboceros_geometry::{NurbsSurface, Point3, Tolerance, WeightedPoint3};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap()
}

#[test]
fn plane_certificates_cover_interior_edges_corners_and_bad_seeds() {
    let source =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(0., 2., 0.)])
            .unwrap();
    for (u, v) in [(0. ..=1., 0. ..=1.), (-1e12..=-1e12 + 4., 1e-9..=5e-9)] {
        let s = source.try_reparameterized(u, v).unwrap();
        let before = s.clone();
        for (query, expected, distance) in [
            (p(1., 1., 3.), p(1., 1., 0.), 3.),
            (p(-3., 1., 4.), p(0., 1., 0.), 5.),
            (p(-3., -4., 0.), p(0., 0., 0.), 5.),
            (p(1., 1., 0.), p(1., 1., 0.), 0.),
        ] {
            for seed in [None, Some([*s.domain_u().end(), *s.domain_v().start()])] {
                let result = if let Some(uv) = seed {
                    s.minimum_distance_to_point_seeded(query, tolerance(), uv)
                } else {
                    s.minimum_distance_to_point(query, tolerance())
                }
                .unwrap()
                .unwrap();
                assert!(result.lower_distance_bound() <= distance);
                assert!(result.upper_distance >= distance);
                assert!(result.upper_distance - distance <= tolerance().absolute());
                assert!(result.point.distance_to(expected).unwrap() < 1e-8);
                assert!(
                    s.domain_u().contains(&result.parameters[0])
                        && s.domain_v().contains(&result.parameters[1])
                );
            }
        }
        assert_eq!(s, before);
    }
}

#[test]
fn rational_cylinder_certifies_radial_distance_under_common_weight_gauges() {
    let w = 0.5_f64.sqrt();
    for gauge in [1., -1., 1e-200, 1e200] {
        let controls = [0., 2.]
            .into_iter()
            .flat_map(|z| {
                [(p(3., 0., z), 1.), (p(3., 3., z), w), (p(0., 3., z), 1.)]
                    .map(|(p, w)| WeightedPoint3::try_new(p, gauge * w).unwrap())
            })
            .collect();
        let s = NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            controls,
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        for (query, expected) in [
            (p(4., 0., 1.), p(3., 0., 1.)),
            (
                p(4. / 2_f64.sqrt(), 4. / 2_f64.sqrt(), 1.),
                p(3. / 2_f64.sqrt(), 3. / 2_f64.sqrt(), 1.),
            ),
        ] {
            let r = s
                .minimum_distance_to_point_seeded(query, tolerance(), [1., 0.])
                .unwrap()
                .unwrap();
            assert!((r.upper_distance - 1.).abs() < 1e-8);
            assert!(r.point.distance_to(expected).unwrap() < 1e-7);
            assert!(r.lower_distance_bound() <= 1. + 1e-14);
        }
    }
}

#[test]
fn the_complete_patch_atlas_overrules_a_local_seed() {
    let controls = [0., 1.]
        .into_iter()
        .flat_map(|z| [p(0., 0., z), p(-2., 0., z), p(-2., 4., z), p(4., 4., z)])
        .collect();
    let s = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        controls,
        vec![0., 0., 1., 2., 3., 3.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let r = s
        .minimum_distance_to_point_seeded(p(1., 1., 0.5), tolerance(), [3., 0.5])
        .unwrap()
        .unwrap();
    assert!(r.point.distance_to(p(0., 0., 0.5)).unwrap() < 1e-9);
    assert!((r.upper_distance - 2_f64.sqrt()).abs() < 1e-9);
    assert!(r.lower_distance_bound() <= 2_f64.sqrt());
    let tiny = Tolerance::try_new(f64::from_bits(1), 1e-12, 1e-10).unwrap();
    assert!(
        s.minimum_distance_to_point_seeded(p(1., 1., 0.5), tiny, [0., 0.5])
            .unwrap()
            .is_none()
    );
}

#[test]
fn constant_surfaces_keep_subnormal_and_huge_distance_arithmetic_exact() {
    let s = NurbsSurface::try_bilinear([p(0., 0., 0.); 4]).unwrap();
    let tiny = f64::from_bits(1);
    let tol = Tolerance::try_new(tiny, 1e-12, 1e-10).unwrap();
    for distance in [0., tiny, 2_f64.powi(1023)] {
        let r = s
            .minimum_distance_to_point_seeded(p(distance, 0., 0.), tol, [0., 0.])
            .unwrap()
            .unwrap();
        assert_eq!(r.upper_distance, distance);
        assert!(r.lower_distance_bound() <= distance);
    }
    assert!(
        s.minimum_distance_to_point_seeded(p(1., 1., 0.), tol, [0., 0.])
            .unwrap()
            .is_none()
    );
    let s = NurbsSurface::try_bilinear([p(-f64::MAX, 0., 0.); 4]).unwrap();
    assert!(
        s.minimum_distance_to_point_seeded(p(f64::MAX, 0., 0.), tolerance(), [0., 0.])
            .unwrap()
            .is_none()
    );
}

#[test]
fn unsupported_denominators_and_invalid_seeds_never_become_certificates() {
    let points = [p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.), p(1., 1., 0.)];
    let s = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        points
            .into_iter()
            .zip([1., -0.1, 1., 1.])
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(
        s.minimum_distance_to_point(p(0., 0., 1.), tolerance())
            .unwrap()
            .is_none()
    );
    for seed in [[f64::NAN, 0.], [0., 2.], [-1., 0.]] {
        assert!(
            s.minimum_distance_to_point_seeded(p(0., 0., 1.), tolerance(), seed)
                .is_err()
        );
    }
}

#[test]
fn unclamped_active_domains_exclude_inactive_control_extrema() {
    let points = [0., 1.]
        .into_iter()
        .flat_map(|y| [0., 1., 2., 3.].map(|x| p(x, y, 0.)))
        .collect();
    let s = NurbsSurface::try_new(
        2,
        1,
        4,
        2,
        points,
        vec![0., 1., 2., 3., 4., 5., 6.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let r = s
        .minimum_distance_to_point_seeded(p(-2.5, 0.5, 4.), tolerance(), [4., 1.])
        .unwrap()
        .unwrap();
    assert!(r.point.distance_to(p(0.5, 0.5, 0.)).unwrap() < 1e-9);
    assert_eq!(r.upper_distance, 5.);
    assert!(r.lower_distance_bound() <= 5.);
}

#[test]
fn representation_limits_are_inconclusive_before_numeric_search() {
    let high = NurbsSurface::try_clamped_uniform(17, 1, 18, 2, vec![p(0., 0., 0.); 36]).unwrap();
    let wide = NurbsSurface::try_clamped_uniform(1, 1, 2049, 2, vec![p(0., 0., 0.); 4098]).unwrap();
    let jump = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| [0., 1., 3., 4.].map(|x| p(x, y, 0.)))
            .collect(),
        vec![0., 0., 1., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for s in [high, wide, jump] {
        assert!(
            s.minimum_distance_to_point(p(0., 0., 0.), tolerance())
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn a_stationary_saddle_seed_does_not_replace_global_minimization() {
    let s = NurbsSurface::try_bilinear([
        p(-1., -1., 2.),
        p(1., -1., -2.),
        p(1., 1., 2.),
        p(-1., 1., -2.),
    ])
    .unwrap();
    let tol = Tolerance::try_new(1e-8, 1e-12, 1e-10).unwrap();
    let r = s
        .minimum_distance_to_point_seeded(p(0., 0., 1.), tol, [0.5, 0.5])
        .unwrap()
        .unwrap();
    // z=2xy has two closest points (+/-0.5,+/-0.5,0.5), each
    // with squared distance 3/4. The origin is a stationary saddle.
    let distance = 0.75_f64.sqrt();
    assert!(r.lower_distance_bound() <= distance && r.upper_distance >= distance);
    assert!(r.upper_distance - distance < tol.absolute());
    assert!((r.point.x().abs() - 0.5).abs() < 1e-3);
    assert!((r.point.y() - r.point.x()).abs() < 1e-3);
}
