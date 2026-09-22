use super::*;
use crate::exact_scalar::{Rational, rational};

fn encloses(bound: Bound, exact: Rational) {
    assert!(
        bound.lo == Real::NEG_INFINITY || rational(bound.lo) <= exact,
        "{bound:?}"
    );
    assert!(
        bound.hi == Real::INFINITY || rational(bound.hi) >= exact,
        "{bound:?}"
    );
}

#[test]
fn outward_arithmetic_encloses_exact_results_across_binary64_range() {
    let values = [
        0.,
        Real::from_bits(1),
        Real::MIN_POSITIVE,
        1e-200,
        0.1,
        1.,
        1.0_f64.next_up(),
        2_f64.powi(53),
        1e200,
        Real::MAX,
    ];
    for x in values.into_iter().flat_map(|x| [x, -x]) {
        for y in values {
            for sign in [-1., 1.] {
                let y = sign * y;
                let a = Bound::point(x);
                let b = Bound::point(y);
                encloses(a + b, rational(x) + rational(y));
                encloses(a - b, rational(x) - rational(y));
                encloses(a * b, rational(x) * rational(y));
                if y != 0. {
                    encloses(a / b, rational(x) / rational(y));
                }
                // Exercise non-point enclosures and each sign configuration,
                // not only the singleton inputs used to start a recurrence.
                let sum = a + b;
                let difference = a - b;
                encloses(
                    sum * difference,
                    (rational(x) + rational(y)) * (rational(x) - rational(y)),
                );
            }
        }
    }
}

#[test]
fn sqrt_bounds_are_verified_by_exact_squares_and_zero_crossings_reject_division() {
    for x in [
        Real::from_bits(1),
        Real::MIN_POSITIVE,
        1e-200,
        0.1,
        1.,
        2.,
        1e200,
        Real::MAX,
    ] {
        let root = Bound::point(x).positive_sqrt().unwrap();
        assert!(rational(root.lo) * rational(root.lo) <= rational(x));
        assert!(rational(root.hi) * rational(root.hi) >= rational(x));
    }
    for denominator in [Bound::point(0.), Bound { lo: -1., hi: 1. }, Bound::WHOLE] {
        let result = Bound::point(1.) / denominator;
        assert_eq!(result.lo, Real::NEG_INFINITY);
        assert_eq!(result.hi, Real::INFINITY);
        assert!(denominator.positive_sqrt().is_none());
    }
}
