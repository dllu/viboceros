use super::*;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn ellipse(a: Real, b: Real) -> Ellipse3 {
    Ellipse3::try_new(
        p(0., 0., 0.),
        a,
        b,
        Vector3::try_new(1., 0., 0.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Vector3::try_new(0., 1., 0.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::try_new(1e-300, 1e-12, 1e-10).unwrap(),
    )
    .unwrap()
}

#[test]
fn exterior_root_matches_independent_high_precision_stationarity_solution() {
    // 65-decimal root of (5 cos(t)+11)(-5 sin(t)) +
    // (2 sin(t)-3)(2 cos(t)) = 0, t=2.9684862492697670407216819…
    let e = ellipse(5., 2.);
    let expected = p(-4.92527231828194, 0.3444863052756997, 0.);
    for height in [0., 1e100] {
        let t = e.closest_parameter(p(-11., 3., height)).unwrap();
        let q = e.evaluate(t).unwrap();
        assert!(q.distance_to(expected).unwrap() < 3e-15, "{q:?}");
    }
}

#[test]
fn axis_queries_medial_ties_center_and_seam_have_analytic_answers() {
    let e = ellipse(5., 2.);
    for (target, expected) in [
        (p(0., 0., 0.), p(0., 2., 0.)),
        (p(10., 0., 7.), p(5., 0., 0.)),
        (p(-10., 0., 7.), p(-5., 0., 0.)),
        (p(0., -10., 7.), p(0., -2., 0.)),
        // On the medial axis, X/a = a*px/(a²-b²) = 5/21.
        (
            p(1., 0., 0.),
            p(25. / 21., 2. * (1. - (5_f64 / 21.).powi(2)).sqrt(), 0.),
        ),
    ] {
        let q = e.evaluate(e.closest_parameter(target).unwrap()).unwrap();
        assert!(
            q.distance_to(expected).unwrap() < 3e-15,
            "{target:?} -> {q:?}"
        );
    }
    let e = ellipse(2., 5.);
    let t = e.closest_parameter(p(0., -1., 0.)).unwrap();
    let q = e.evaluate(t).unwrap();
    assert!(q.x() < 0. && q.y() < 0.); // earlier of the two reflected minima
    assert_eq!(
        ellipse(2., 2.).closest_parameter(p(0., 0., 9.)).unwrap(),
        0.
    );
}

#[test]
fn reflections_domains_reversal_and_wide_uniform_scales_preserve_the_locus() {
    for scale in [1e-150, 1., 1e150] {
        for e in [
            ellipse(5. * scale, 2. * scale),
            ellipse(2. * scale, 5. * scale),
        ] {
            for (x, y) in [(-11., 3.), (11., 3.), (11., -3.), (-11., -3.), (0.2, 0.3)] {
                let target = p(x * scale, y * scale, 0.);
                let q = e.evaluate(e.closest_parameter(target).unwrap()).unwrap();
                let unit = [q.x() / e.radius_x(), q.y() / e.radius_y()];
                assert!((unit[0] * unit[0] + unit[1] * unit[1] - 1.).abs() < 2e-15);
                // Independent stationarity and exhaustive coarse competitor grid.
                let tangent = [
                    -e.radius_x() / scale * unit[1],
                    e.radius_y() / scale * unit[0],
                ];
                let residual = [q.x() / scale - x, q.y() / scale - y];
                assert!((residual[0] * tangent[0] + residual[1] * tangent[1]).abs() < 1e-12);
                let distance = residual[0].hypot(residual[1]);
                for i in 0..720 {
                    let angle = std::f64::consts::TAU * i as Real / 720.;
                    let other = (e.radius_x() / scale * angle.cos() - x)
                        .hypot(e.radius_y() / scale * angle.sin() - y);
                    assert!(distance <= other + 1e-13);
                }
                for variant in [e.try_reparameterized(-7.0..=13.0).unwrap(), e.reversed()] {
                    let t = variant.closest_parameter(target).unwrap();
                    assert!(variant.domain().contains(&t));
                    assert!(variant.evaluate(t).unwrap().distance_to(q).unwrap() / scale < 3e-14);
                }
            }
        }
    }
}

#[test]
fn coefficient_underflow_is_an_explicit_failure() {
    assert!(
        ellipse(1e-200, 1.)
            .closest_parameter(p(1e-200, 1., 0.))
            .is_err()
    );
}

#[test]
fn near_evolute_axis_queries_do_not_lose_the_small_stationary_offset() {
    let e = ellipse(5., 2.);
    assert_eq!(e.closest_parameter(p(4.2, 0., 0.)).unwrap(), 0.);
    let x = 4.2_f64.next_down();
    // Exact-input cosine = 5*x/21. The stable sine uses the small exact
    // difference from one rather than subtracting rounded cos² from one.
    let c = rational(5.) * rational(x) / rational(21.);
    let s = scalar(&((rational(1.) - &c) * (rational(1.) + &c)))
        .unwrap()
        .sqrt();
    let expected = p(5. * scalar(&c).unwrap(), 2. * s, 0.);
    let q = e
        .evaluate(e.closest_parameter(p(x, 0., 0.)).unwrap())
        .unwrap();
    assert!(q.y() > 0.);
    assert!(
        q.distance_to(expected).unwrap() < 1e-15,
        "{q:?} != {expected:?}"
    );
}
