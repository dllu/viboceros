use super::*;

#[test]
fn exact_fractional_samples_match_independent_basis_sum_bits() {
    let mut count = 0;
    let mut poles = 0;
    let mut overflows = 0;
    for line in include_str!("reference.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
    {
        let mut fields = line.split_whitespace();
        let degree: usize = fields.next().unwrap().parse().unwrap();
        let control_count: usize = fields.next().unwrap().parse().unwrap();
        let span: isize = fields.next().unwrap().parse().unwrap();
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
        let actual = if span < 0 {
            let a = *sampler.curve.domain().start();
            let b = *sampler.curve.domain().end();
            assert!(
                sampler.exact_domain
                    || lost_station_range(
                        a,
                        b,
                        fraction,
                        sample_parameter(a, b, fraction).unwrap()
                    )
                    || sampler
                        .curve
                        .evaluate(sample_parameter(a, b, fraction).unwrap())
                        .is_err()
            );
            sampler.evaluate(fraction)
        } else {
            let span = sampler.spans().find(|s| s.span == span as usize).unwrap();
            assert!(
                span.exact_interval
                    || lost_station_range(
                        span.start,
                        span.end,
                        fraction,
                        sample_parameter(span.start, span.end, fraction).unwrap()
                    )
                    || span
                        .curve
                        .evaluate(sample_parameter(span.start, span.end, fraction).unwrap())
                        .is_err()
            );
            span.evaluate(fraction)
        };
        let expected: Vec<_> = fields.collect();
        if expected == ["pole"] {
            assert_eq!(actual, Err(GeometryError::ZeroWeightAtParameter));
            poles += 1;
        } else {
            assert_eq!(expected.len(), 3);
            let expected: Vec<_> = expected.iter().map(|s| parse(s)).collect();
            if expected.iter().any(|v| !v.is_finite()) {
                assert!(
                    matches!(actual, Err(GeometryError::NonFinite { .. })),
                    "case {count}: {actual:?}"
                );
                overflows += 1;
            } else {
                assert_eq!(
                    actual.unwrap().to_array().map(Real::to_bits).as_slice(),
                    expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    "case {count}"
                );
            }
        }
        count += 1;
    }
    assert_eq!(count, 173);
    assert!(poles > 0);
    assert!(overflows > 0);
}
