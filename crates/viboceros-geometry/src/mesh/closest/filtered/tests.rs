//! Scalar certificates must enclose exact arithmetic, including residual floors.
use super::*;
use crate::exact_scalar::rational;
use num_traits::Zero;

#[test]
fn scalar_enclosures_contain_exact_results_across_the_finite_range() {
    let mut pairs = vec![
        (f64::MAX, -f64::MAX),
        (f64::MAX, f64::MAX),
        (RESIDUAL_FLOOR, 1. + f64::EPSILON),
        (RESIDUAL_FLOOR.next_down(), 1. + f64::EPSILON),
        (f64::MIN_POSITIVE, 0.5),
        (1., 3.),
        (1e16, 1e16 - 2.),
        (0., 0.),
    ];
    let mut state = 0x13a5_290d_bc16_0ef9u64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut bits = state;
        if bits & (0x7ff << 52) == 0x7ff << 52 {
            bits ^= 1 << 52;
        }
        f64::from_bits(bits)
    };
    for _ in 0..1024 {
        pairs.push((next(), next()));
    }
    let mut admitted = 0;
    for (a, b) in pairs {
        let (Some(aa), Some(bb)) = (F::point(a), F::point(b)) else {
            continue;
        };
        let (cc, dd) = (F::point(aa.value.next_down()), F::point(bb.value.next_up()));
        let (a, b) = (rational(a), rational(b));
        let mut checks = vec![
            ("add", aa.add(bb), Some(&a + &b)),
            ("sub", aa.sub(bb), Some(&a - &b)),
            ("mul", aa.mul(bb), Some(&a * &b)),
            ("div", aa.div(bb), (!b.is_zero()).then(|| &a / &b)),
        ];
        if let (Some(c), Some(d)) = (cc, dd) {
            checks.push((
                "product difference",
                F::product_sum(aa, bb, c.neg(), d),
                Some(&a * &b - rational(c.value) * rational(d.value)),
            ));
        }
        for (operation, result, expected) in checks {
            if let (Some(result), Some(expected)) = (result, expected) {
                assert!(result.interval.lo <= result.value && result.value <= result.interval.hi);
                assert!(
                    rational(result.interval.lo) <= expected
                        && rational(result.interval.hi) >= expected,
                    "{operation}: {:?}, {:?}: {:?}",
                    aa.value,
                    bb.value,
                    result.interval
                );
                admitted += 1;
            }
        }
    }
    assert!(admitted > 2000);
}
