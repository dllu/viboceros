use super::*;

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}

fn shifted(curve: &NurbsCurve, origin: Real) -> NurbsCurve {
    let knots: Vec<_> = curve.knots().iter().map(|k| k + origin).collect();
    assert!(
        knots
            .iter()
            .zip(curve.knots())
            .all(|(a, b)| a - origin == *b)
    );
    NurbsCurve::try_new_rational(curve.degree(), curve.control_points().to_vec(), knots).unwrap()
}

#[test]
fn rational_fraction_samples_preserve_locus_controls_and_native_domain() {
    for sign in [1., -1.] {
        let curve = NurbsCurve::try_new_rational(
            2,
            [(p(0., 0.), 1.), (p(1., 2.), 2.), (p(2., 0.), 1.)]
                .map(|(p, w)| WeightedPoint3::try_new(p, sign * w).unwrap())
                .to_vec(),
            vec![0., 0., 0., 2., 2., 2.],
        )
        .unwrap();
        for origin in [0., 1e12, -1e12, 2.0_f64.powi(52), -2.0_f64.powi(52)] {
            let translated = shifted(&curve, origin);
            let original = translated.clone();
            let sampler = translated.parameter_sampler().unwrap();
            assert_eq!(sampler.curve.control_points(), translated.control_points());
            for t in [0., 1. / 7., 1. / 3., 0.5, 2. / 3., 6. / 7., 1.] {
                let a = 1. - t;
                let denominator = a * a + 4. * a * t + t * t;
                let expected = p(
                    (4. * a * t + 2. * t * t) / denominator,
                    8. * a * t / denominator,
                );
                let actual = sampler.evaluate(t).unwrap();
                assert!(
                    actual.distance_to(expected).unwrap() < 2e-15,
                    "origin={origin}, t={t}: {actual:?} != {expected:?}"
                );
                assert_eq!(actual, sampler.spans().next().unwrap().evaluate(t).unwrap());
            }
            // The native API still means the exact f64 parameter passed by
            // its caller, not an inferred fraction that was rounded away.
            if origin == 2.0_f64.powi(52) {
                let native = translated
                    .evaluate(translated.parameter_at(1. / 3.).unwrap())
                    .unwrap();
                assert!(
                    sampler
                        .evaluate(1. / 3.)
                        .unwrap()
                        .distance_to(native)
                        .unwrap()
                        > 0.1
                );
            }
            for t in [*translated.domain().start(), *translated.domain().end()] {
                assert_eq!(
                    translated.evaluate(t).unwrap(),
                    original.evaluate(t).unwrap()
                );
            }
            assert_eq!(translated, original);
        }
    }
}

#[test]
fn unclamped_multispan_samples_and_reversed_curves_retain_geometry() {
    let curve = NurbsCurve::try_new_rational(
        2,
        [
            (p(0., 0.), 1.),
            (p(1., 2.), 2.),
            (p(2., -1.), 0.5),
            (p(4., 1.), 4.),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![-2., -1., 0., 1., 2., 3., 4.],
    )
    .unwrap();
    for origin in [1e12, -1e12, 2.0_f64.powi(50), -2.0_f64.powi(50)] {
        for reversed in [false, true] {
            let (baseline, translated) = if reversed {
                (
                    curve.reversed().unwrap(),
                    shifted(&curve, origin).reversed().unwrap(),
                )
            } else {
                (curve.clone(), shifted(&curve, origin))
            };
            let expected = baseline.parameter_sampler().unwrap();
            let actual = translated.parameter_sampler().unwrap();
            assert_eq!(expected.spans().count(), 2);
            assert_eq!(actual.spans().count(), 2);
            for (a, b) in actual.spans().zip(expected.spans()) {
                for i in 0..=17 {
                    assert!(
                        a.evaluate(i as Real / 17.)
                            .unwrap()
                            .distance_to(b.evaluate(i as Real / 17.).unwrap())
                            .unwrap()
                            < 4e-15
                    );
                }
            }
        }
    }
}

#[test]
fn span_endpoints_are_true_one_sided_limits() {
    let controls = [p(-4., 0.), p(-2., 0.), p(2., 0.), p(4., 0.)];
    let curve = NurbsCurve::try_new(1, controls.to_vec(), vec![0., 0., 1., 1., 2., 2.]).unwrap();
    for origin in [0., 2.0_f64.powi(52), -2.0_f64.powi(52)] {
        let translated = shifted(&curve, origin);
        let sampler = translated.parameter_sampler().unwrap();
        let spans: Vec<_> = sampler.spans().collect();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].evaluate(0.).unwrap(), controls[0]);
        assert_eq!(spans[0].evaluate(1.).unwrap(), controls[1]);
        assert_eq!(spans[1].evaluate(0.).unwrap(), controls[2]);
        assert_eq!(spans[1].evaluate(1.).unwrap(), controls[3]);
        assert_eq!(sampler.evaluate(0.5).unwrap(), controls[2]);
        assert_eq!(spans[0].evaluate(1. / 3.).unwrap(), p(-4. + 2. / 3., 0.));
    }
}

#[test]
fn closed_predicate_does_not_round_interior_stations_to_the_seam() {
    let curve = NurbsCurve::try_new(
        3,
        vec![p(0., 0.), p(1., 1.), p(-1., 1.), p(0., 0.)],
        vec![0., 0., 0., 0., 2., 2., 2., 2.],
    )
    .unwrap();
    assert!(curve.is_closed().unwrap());
    for origin in [2.0_f64.powi(53), -2.0_f64.powi(53)] {
        assert!(shifted(&curve, origin).is_closed().unwrap());
    }
}

#[test]
fn sampler_borrows_identity_and_declines_inexact_exterior_knots() {
    for knots in [
        vec![0., 0., 1., 1.],
        vec![-2., -2., 1., 1.],
        vec![-1e308, 1., 2., 3.],
    ] {
        let curve = NurbsCurve::try_new(1, vec![p(0., 0.), p(1., 1.)], knots).unwrap();
        let sampler = curve.parameter_sampler().unwrap();
        assert!(matches!(sampler.curve, Cow::Borrowed(_)));
        assert_eq!(&*sampler.curve, &curve);
        for t in [0., 1. / 3., 1.] {
            assert_eq!(
                sampler.evaluate(t).unwrap(),
                curve.evaluate(curve.parameter_at(t).unwrap()).unwrap()
            );
        }
    }
}

#[test]
fn unshiftable_span_keeps_rounded_stations_on_its_own_side() {
    let origin = 2.0_f64.powi(52);
    // The exterior knot prevents a lossless frame shift, while each active
    // interval has no representable interior native parameter.
    let curve = NurbsCurve::try_new(
        1,
        vec![p(-4., 0.), p(-2., 0.), p(2., 0.), p(4., 0.)],
        vec![
            -1e308,
            origin,
            origin + 1.,
            origin + 1.,
            origin + 2.,
            origin + 2.,
        ],
    )
    .unwrap();
    let sampler = curve.parameter_sampler().unwrap();
    assert!(matches!(sampler.curve, Cow::Borrowed(_)));
    let first = sampler.spans().next().unwrap();
    for i in 0..=31 {
        assert!(first.evaluate(i as Real / 31.).unwrap().x() < 0.);
    }
}

#[test]
fn sampling_retains_exact_recovery_and_zero_weight_errors() {
    for weights in [
        [Real::from_bits(1), Real::MAX],
        [Real::MAX, Real::from_bits(1)],
        [1., -1.],
    ] {
        let curve = NurbsCurve::try_new_rational(
            1,
            [p(1e300, -1e300), p(-1e300, 1e300)]
                .into_iter()
                .zip(weights)
                .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
                .collect(),
            vec![0., 0., 2., 2.],
        )
        .unwrap();
        let translated = shifted(&curve, 2.0_f64.powi(52));
        let sampler = translated.parameter_sampler().unwrap();
        for fraction in [0., 0.125, 1. / 3., 0.5, 0.75, 1.] {
            assert_eq!(sampler.evaluate(fraction), curve.evaluate(2. * fraction));
        }
        if weights == [1., -1.] {
            assert_eq!(
                sampler.evaluate(0.5),
                Err(GeometryError::ZeroWeightAtParameter)
            );
        }
    }
}

#[test]
fn fractions_are_checked_and_extreme_domain_endpoints_stay_exact() {
    for knots in [
        vec![0., 0., Real::from_bits(1), Real::from_bits(1)],
        vec![-Real::MAX, -Real::MAX, Real::MAX, Real::MAX],
    ] {
        let curve = NurbsCurve::try_new(1, vec![p(0., 0.), p(1., 1.)], knots).unwrap();
        let sampler = curve.parameter_sampler().unwrap();
        let span = sampler.spans().next().unwrap();
        for t in [
            Real::NAN,
            Real::NEG_INFINITY,
            Real::INFINITY,
            -Real::from_bits(1),
            1.0_f64.next_up(),
        ] {
            assert!(sampler.evaluate(t).is_err());
            assert!(span.evaluate(t).is_err());
        }
        assert_eq!(sampler.evaluate(0.).unwrap(), p(0., 0.));
        assert_eq!(sampler.evaluate(1.).unwrap(), p(1., 1.));
        assert_eq!(span.evaluate(0.).unwrap(), p(0., 0.));
        assert_eq!(span.evaluate(1.).unwrap(), p(1., 1.));
    }
}
