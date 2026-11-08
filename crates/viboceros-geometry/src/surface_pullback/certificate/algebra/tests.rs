use super::*;

fn verify(p: H, limit: Real) {
    let squared_length = p[..3].iter().map(|x| x * x).sum::<Rational>();
    let squared_weight = &p[3] * &p[3];
    let admissible = squared_length <= rational(limit) * rational(limit) * &squared_weight;
    let result = norm_bound(&p, limit);
    assert_eq!(result.is_some(), admissible);
    if let Some(upper) = result {
        assert!(upper.is_finite() && upper >= 0. && upper <= limit);
        assert!(rational(upper) * rational(upper) * squared_weight >= squared_length);
    }
}

#[test]
fn interval_screen_decides_separated_norms_and_preserves_boundary_fallback() {
    let p = [rational(3.), rational(4.), rational(0.), rational(1.)];
    assert!(matches!(
        interval::rational_norm(&p, 6.),
        interval::Outcome::Within(_)
    ));
    assert!(matches!(
        interval::rational_norm(&p, 4.),
        interval::Outcome::Outside
    ));
    assert!(matches!(
        interval::rational_norm(&p, 5.),
        interval::Outcome::Inconclusive
    ));
    for limit in [4., 5_f64.next_down(), 5., 5_f64.next_up(), 6.] {
        verify(p.clone(), limit);
    }
}

#[test]
fn zero_subnormal_and_overflow_intermediates_keep_exact_decisions() {
    let tiny = Real::from_bits(1);
    for limit in [0., tiny, tiny * 2., Real::MIN_POSITIVE] {
        for value in [0., tiny, tiny * 2.] {
            verify(
                [rational(value), rational(0.), rational(0.), rational(1.)],
                limit,
            );
        }
    }
    let huge = Rational::from_integer(BigInt::one() << 2048);
    let p = [huge.clone(), Rational::zero(), Rational::zero(), huge];
    assert!(matches!(
        interval::rational_norm(&p, 1.),
        interval::Outcome::Inconclusive
    ));
    verify(p, 1.);
    verify(
        [
            rational(Real::MAX),
            rational(Real::MAX),
            rational(0.),
            rational(Real::MAX),
        ],
        2.,
    );
    verify(
        [
            rational(Real::MAX),
            rational(Real::MAX),
            rational(0.),
            rational(1.),
        ],
        Real::MAX,
    );
}

#[test]
fn large_denominators_and_signed_gauges_satisfy_independent_squared_bounds() {
    for bits in [64, 512, 2048] {
        let base = BigInt::one() << bits;
        for sign in [-1., 1.] {
            let weight = rational(sign) * Rational::new(&base + 13, &base + 17);
            for limit in [1e-300, 1e-7, 1., 1e300] {
                for coordinates in [[0.2, -0.3, 0.1], [0.7, 0.7, 0.7], [1., 0., 0.]] {
                    let p = std::array::from_fn(|i| {
                        if i == 3 {
                            weight.clone()
                        } else {
                            rational(coordinates[i] * limit)
                                * &weight
                                * Rational::new(&base + (19 + i), &base + (23 + i))
                        }
                    });
                    verify(p, limit);
                }
            }
        }
    }
}
