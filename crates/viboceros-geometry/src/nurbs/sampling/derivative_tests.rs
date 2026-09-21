use super::*;

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}

fn rational_line(start: Real, end: Real, gauge: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0.), gauge).unwrap(),
            WeightedPoint3::try_new(p(2., 0.), 2. * gauge).unwrap(),
        ],
        vec![start, start, end, end],
    )
    .unwrap()
}

fn close(actual: Real, expected: Real) {
    assert!(
        (actual - expected).abs() <= 2e-14 * expected.abs().max(1.),
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn fractional_derivatives_ignore_native_domain_speed_and_weight_gauge() {
    let intervals = [
        (0., Real::from_bits(1)),
        (0., Real::MIN_POSITIVE / 2.),
        (0., 1e-170),
        (0., 1.),
        (0., 1e170),
        (0., 1e308),
        (-1e308, 1e308),
        (1e12, 1e12 + 2.),
        (-1e12, -1e12 + 2.),
        (2_f64.powi(52), 2_f64.powi(52) + 1.),
    ];
    for (start, end) in intervals {
        for gauge in [1., -1., 1e-200, -1e200, Real::from_bits(1)] {
            let curve = rational_line(start, end, gauge);
            let unchanged = curve.clone();
            let sampler = curve.parameter_sampler().unwrap();
            let span = sampler.spans().next().unwrap();
            for f in [0., 0.125, 1. / 3., 0.5, 0.875, 1.] {
                for (point, derivative) in [
                    sampler.evaluate_with_derivative(f).unwrap(),
                    span.evaluate_with_derivative(f).unwrap(),
                ] {
                    close(point.x(), 4. * f / (1. + f));
                    close(derivative.x(), 4. / (1. + f).powi(2));
                    assert_eq!([derivative.y(), derivative.z()], [0.; 2]);
                }
            }
            assert_eq!(curve, unchanged);
        }
    }
}

#[test]
fn span_and_whole_derivatives_have_distinct_scales_and_sided_limits() {
    let curve = NurbsCurve::try_new(
        1,
        vec![p(-4., 0.), p(-2., 0.), p(2., 0.), p(8., 0.)],
        vec![0., 0., 1., 1., 3., 3.],
    )
    .unwrap();
    let sampler = curve.parameter_sampler().unwrap();
    let spans: Vec<_> = sampler.spans().collect();
    for f in [0., 0.3, 1.] {
        let (a, da) = spans[0].evaluate_with_derivative(f).unwrap();
        let (b, db) = spans[1].evaluate_with_derivative(f).unwrap();
        close(a.x(), -4. + 2. * f);
        close(b.x(), 2. + 6. * f);
        assert_eq!(da.to_array(), [2., 0., 0.]);
        assert_eq!(db.to_array(), [6., 0., 0.]);
    }
    // Binary64 one third lies just before the discontinuity, not on it.
    let before = sampler.evaluate_with_derivative(1. / 3.).unwrap();
    let after = sampler
        .evaluate_with_derivative((1. / 3_f64).next_up())
        .unwrap();
    assert_eq!(before.0, p(-2., 0.));
    close(after.0.x(), 2.);
    assert_eq!(before.1.to_array(), [6., 0., 0.]);
    assert_eq!(after.1.to_array(), [9., 0., 0.]);
}

#[test]
fn unshiftable_knots_and_adjacent_float_spans_keep_fractional_tangents() {
    let origin = 2_f64.powi(52);
    let curve = NurbsCurve::try_new(
        1,
        vec![p(-4., 0.), p(-2., 0.), p(2., 0.), p(8., 0.)],
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
    for (index, span) in sampler.spans().enumerate() {
        for f in [0., 0.3, 1.] {
            let (point, derivative) = span.evaluate_with_derivative(f).unwrap();
            close(
                point.x(),
                if index == 0 {
                    -4. + 2. * f
                } else {
                    2. + 6. * f
                },
            );
            assert_eq!(derivative.x(), if index == 0 { 2. } else { 6. });
        }
    }
}

#[test]
fn translated_quadratic_retains_normalized_derivatives_and_stationary_points() {
    for origin in [0., 1e12, -1e12] {
        for width in [Real::from_bits(1), 1e-170, 2., 1e170] {
            let curve = NurbsCurve::try_new(
                2,
                vec![p(origin, 0.), p(origin + 2., 4.), p(origin + 4., 0.)],
                vec![0., 0., 0., width, width, width],
            )
            .unwrap();
            let sampler = curve.parameter_sampler().unwrap();
            for f in [0., 0.25, 0.5, 0.75, 1.] {
                let (point, derivative) = sampler.evaluate_with_derivative(f).unwrap();
                assert_eq!(point.x(), origin + 4. * f);
                close(point.y(), 8. * f * (1. - f));
                close(derivative.x(), 4.);
                close(derivative.y(), 8. - 16. * f);
            }
        }
    }
}

#[test]
fn subnormal_and_overflowing_derivative_results_are_not_confused_with_native_speed() {
    let tiny = Real::from_bits(1);
    let curve =
        NurbsCurve::try_new(1, vec![p(0., 0.), p(tiny, 0.)], vec![0., 0., 1e308, 1e308]).unwrap();
    let (point, derivative) = curve
        .parameter_sampler()
        .unwrap()
        .evaluate_with_derivative(0.75)
        .unwrap();
    assert_eq!(point.x(), tiny);
    assert_eq!(derivative.x(), tiny);
    let curve = NurbsCurve::try_new(
        1,
        vec![p(-1e308, 0.), p(1e308, 0.)],
        vec![0., 0., 1e308, 1e308],
    )
    .unwrap();
    assert_eq!(
        curve.parameter_sampler().unwrap().evaluate(0.5).unwrap(),
        p(0., 0.)
    );
    assert!(matches!(
        curve
            .parameter_sampler()
            .unwrap()
            .evaluate_with_derivative(0.5),
        Err(GeometryError::NonFinite { .. })
    ));
}

#[test]
fn signed_weight_poles_and_rounded_native_poles_are_resolved_before_derivatives() {
    for end in [1.5, Real::from_bits(3)] {
        let curve = NurbsCurve::try_new_rational(
            1,
            vec![
                WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
                WeightedPoint3::try_new(p(1., 0.), -2.).unwrap(),
            ],
            vec![0., 0., end, end],
        )
        .unwrap();
        let sampler = curve.parameter_sampler().unwrap();
        let (point, derivative) = sampler.evaluate_with_derivative(1. / 3.).unwrap();
        assert_eq!(point.x(), -12_009_599_006_321_322.);
        assert_eq!(derivative.x(), -2_f64.powi(109));
        assert_eq!(
            sampler
                .spans()
                .next()
                .unwrap()
                .evaluate_with_derivative(1. / 3.)
                .unwrap(),
            (point, derivative)
        );
    }
    let curve = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(1., 0.), -1.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        curve
            .parameter_sampler()
            .unwrap()
            .evaluate_with_derivative(0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    );
}

#[test]
fn constant_and_stationary_spans_return_zero_derivatives_not_arbitrary_tangents() {
    for controls in [vec![p(1e12, 0.); 3], vec![p(0., 0.), p(1., 0.), p(0., 0.)]] {
        let curve = NurbsCurve::try_new(2, controls, vec![0., 0., 0., 1., 1., 1.]).unwrap();
        assert_eq!(
            curve
                .parameter_sampler()
                .unwrap()
                .evaluate_with_derivative(0.5)
                .unwrap()
                .1
                .to_array(),
            [0.; 3]
        );
    }
}

#[test]
fn fractional_derivative_rejects_invalid_fractions_before_evaluation() {
    let curve = rational_line(0., 1., 1.);
    let sampler = curve.parameter_sampler().unwrap();
    let span = sampler.spans().next().unwrap();
    for fraction in [Real::NAN, Real::INFINITY, -Real::INFINITY, -0.1, 1.1] {
        assert!(sampler.evaluate_with_derivative(fraction).is_err());
        assert!(span.evaluate_with_derivative(fraction).is_err());
    }
}

#[test]
fn near_stationary_derivative_recovers_residual_lost_in_homogeneous_blending() {
    // C'(f) = 1 - 3f. At binary64 one third the exact derivative is 2^-54,
    // whereas a rounded complement in the derivative blend doubles it.
    for width in [1., 1.5, 1e170] {
        let curve = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(0.5, 0.), p(-0.5, 0.)],
            vec![0., 0., 0., width, width, width],
        )
        .unwrap();
        let sampler = curve.parameter_sampler().unwrap();
        let result = sampler.evaluate_with_derivative(1. / 3.).unwrap();
        assert_eq!(result.1.x(), 2_f64.powi(-54));
        assert_eq!(
            sampler
                .spans()
                .next()
                .unwrap()
                .evaluate_with_derivative(1. / 3.)
                .unwrap(),
            result
        );
    }
}

#[test]
fn fractional_jets_match_independent_rational_basis_derivatives() {
    let mut count = 0;
    let mut poles = 0;
    let mut overflows = 0;
    for line in include_str!("derivative_reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let mut fields = line.split_whitespace();
        let degree: usize = fields.next().unwrap().parse().unwrap();
        let control_count: usize = fields.next().unwrap().parse().unwrap();
        let span_index: isize = fields.next().unwrap().parse().unwrap();
        let parse = |s: &str| Real::from_bits(u64::from_str_radix(s, 16).unwrap());
        let knots = (0..control_count + degree + 1)
            .map(|_| parse(fields.next().unwrap()))
            .collect();
        let fraction = parse(fields.next().unwrap());
        let controls = (0..control_count)
            .map(|_| {
                let [x, y, z, w] = std::array::from_fn(|_| parse(fields.next().unwrap()));
                WeightedPoint3::try_new(Point3::try_new(x, y, z).unwrap(), w).unwrap()
            })
            .collect();
        let curve = NurbsCurve::try_new_rational(degree, controls, knots).unwrap();
        let sampler = curve.parameter_sampler().unwrap();
        let (public, recovered) = if span_index < 0 {
            (
                sampler.evaluate_with_derivative(fraction),
                exact::first(
                    &curve,
                    *curve.domain().start(),
                    *curve.domain().end(),
                    fraction,
                    None,
                ),
            )
        } else {
            let span = sampler
                .spans()
                .find(|s| s.span == span_index as usize)
                .unwrap();
            (
                span.evaluate_with_derivative(fraction),
                exact::first(
                    &curve,
                    curve.knots[span_index as usize],
                    curve.knots[span_index as usize + 1],
                    fraction,
                    Some(span_index as usize),
                ),
            )
        };
        let expected: Vec<_> = fields.collect();
        if expected == ["pole"] {
            assert_eq!(
                public,
                Err(GeometryError::ZeroWeightAtParameter),
                "case {count}"
            );
            assert_eq!(recovered, Err(GeometryError::ZeroWeightAtParameter));
            poles += 1;
        } else {
            assert_eq!(expected.len(), 6);
            let expected: Vec<_> = expected.iter().map(|s| parse(s)).collect();
            if expected.iter().any(|v| !v.is_finite()) {
                assert!(
                    matches!(public, Err(GeometryError::NonFinite { .. })),
                    "case {count}: {public:?}"
                );
                assert!(matches!(recovered, Err(GeometryError::NonFinite { .. })));
                overflows += 1;
            } else {
                let (p, d) = recovered.unwrap();
                assert_eq!(
                    p.to_array()
                        .into_iter()
                        .chain(d.to_array())
                        .map(Real::to_bits)
                        .collect::<Vec<_>>(),
                    expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    "exact case {count}"
                );
                let (p, d) = public.unwrap();
                for (values, expected) in [p.to_array(), d.to_array()]
                    .into_iter()
                    .zip(expected.chunks(3))
                {
                    let scale = expected.iter().map(|v| v.abs()).fold(0., Real::max);
                    for (v, e) in values.into_iter().zip(expected) {
                        assert!(
                            (v - e).abs() <= (2e-13 * scale).max(Real::from_bits(1)),
                            "public case {count}: {v:?} != {e:?}"
                        );
                    }
                }
            }
        }
        count += 1;
    }
    assert_eq!(count, 493);
    assert!(poles > 0 && overflows > 0);
}
