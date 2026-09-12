//! Allocation-free fallback for dot products outside ordinary floating-point range.
//!
//! Every finite binary64 value is an integer significand times a power of two.
//! Products therefore fit an integer accumulator with quantum 2^-2148. Up to six
//! products need at most 4199 bits, including carry; 66 limbs provide 4224.
//! Separate positive/negative magnitudes avoid losing small terms before large
//! terms cancel. Only the final conversion rounds (nearest, ties to even).

const LIMBS: usize = 66;
use crate::binary_accumulator::{add_product, decompose, finish};

/// Scale the exact sum before rounding. Three binary64 factors need quantum
/// 2^-3222 and at most 6297 bits for six products; 99 limbs provide 6336.
pub(super) fn scaled_dot<const N: usize>(left: [f64; N], right: [f64; N], scale: f64) -> f64 {
    assert!(N <= 6, "exact scaled dot accumulator capacity");
    let mut positive = [0; 99];
    let mut negative = [0; 99];
    let (c, c_shift) = decompose(scale);
    for (a, b) in left.into_iter().zip(right) {
        let (a_bits, a_shift) = decompose(a);
        let (b_bits, b_shift) = decompose(b);
        let product = u128::from(a_bits) * u128::from(b_bits);
        let target = if a.is_sign_negative() ^ b.is_sign_negative() ^ scale.is_sign_negative() {
            &mut negative
        } else {
            &mut positive
        };
        let shift = a_shift + b_shift + c_shift;
        add_product(target, u128::from(product as u64) * u128::from(c), shift);
        add_product(target, (product >> 64) * u128::from(c), shift + 64);
    }
    finish::<99, 3222>(positive, negative)
}

pub(super) fn dot<const N: usize>(left: [f64; N], right: [f64; N]) -> f64 {
    dot_with_quantum::<N, 2148>(left, right)
}

/// Divide the exact sum by two before rounding, including at overflow boundaries.
pub(super) fn half_dot<const N: usize>(left: [f64; N], right: [f64; N]) -> f64 {
    dot_with_quantum::<N, 2149>(left, right)
}

fn dot_with_quantum<const N: usize, const QUANTUM: usize>(left: [f64; N], right: [f64; N]) -> f64 {
    assert!(N <= 6, "exact dot accumulator capacity");
    let mut positive = [0; LIMBS];
    let mut negative = [0; LIMBS];
    for (a, b) in left.into_iter().zip(right) {
        let (a_significand, a_shift) = decompose(a);
        let (b_significand, b_shift) = decompose(b);
        let product = u128::from(a_significand) * u128::from(b_significand);
        let target = if a.is_sign_negative() != b.is_sign_negative() {
            &mut negative
        } else {
            &mut positive
        };
        add_product(target, product, a_shift + b_shift);
    }
    finish::<LIMBS, QUANTUM>(positive, negative)
}

#[cfg(test)]
mod tests {
    use super::dot;

    #[test]
    fn scaled_dot_preserves_overflow_cancellation_and_subnormal_products() {
        use super::scaled_dot;
        assert_eq!(scaled_dot([f64::MAX, f64::MAX], [1., 1.], 0.5), f64::MAX);
        assert_eq!(
            scaled_dot([f64::MAX, -f64::MAX, 1.], [f64::MAX, f64::MAX, 1.], 0.25),
            0.25
        );
        assert_eq!(
            scaled_dot([f64::from_bits(1)], [0.5], 2.),
            f64::from_bits(1)
        );
        assert_eq!(scaled_dot([f64::MAX], [2.], 1.), f64::INFINITY);
        assert_eq!(
            scaled_dot([f64::MAX], [f64::MAX], -f64::MAX),
            f64::NEG_INFINITY
        );
        assert_eq!(scaled_dot([2.], [-3.], -0.5), 3.);
        assert_eq!(scaled_dot([f64::MAX], [f64::MAX], 0.), 0.);
        for a in -30_i128..=30 {
            for b in -30_i128..=30 {
                let expected = (a * b - (a + 1) * (b - 1)) * 17;
                assert_eq!(
                    scaled_dot([a as f64, -(a + 1) as f64], [b as f64, (b - 1) as f64], 17.),
                    expected as f64
                );
            }
        }
    }

    #[test]
    fn half_dot_applies_scaling_before_final_overflow_and_underflow_rounding() {
        let huge = 2.0f64.powi(512);
        assert_eq!(super::half_dot([huge], [huge]), 2.0f64.powi(1023));
        assert_eq!(super::half_dot([huge, -huge], [huge, huge]), 0.0);
        let tiny = f64::from_bits(1);
        assert_eq!(super::half_dot([tiny], [1.0]), 0.0);
        assert_eq!(super::half_dot([tiny, tiny], [1.0, 1.0]), tiny);
        assert_eq!(
            super::half_dot([tiny, tiny, tiny], [1.0; 3]),
            f64::from_bits(2)
        );
        assert_eq!(super::half_dot([f64::MAX], [4.0]), f64::INFINITY);
    }

    #[test]
    fn six_products_preserve_cancellation_and_integer_sums() {
        let huge = 2_f64.powi(1023);
        for small in [f64::from_bits(1), 1., 3.] {
            assert_eq!(dot([huge, -huge, small, huge, -huge, 0.], [1.; 6]), small);
        }
        let mut state = 17_u64;
        for _ in 0..1000 {
            let mut next = || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                i128::from(state & ((1_u64 << 40) - 1)) - (1_i128 << 39)
            };
            let a: [_; 6] = std::array::from_fn(|_| next());
            let b: [_; 6] = std::array::from_fn(|_| next());
            let expected = a.into_iter().zip(b).map(|(a, b)| a * b).sum::<i128>();
            assert_eq!(
                dot(a.map(|v| v as f64), b.map(|v| v as f64)),
                expected as f64
            );
        }
    }

    #[test]
    fn agrees_with_hardware_product_and_fused_add_across_binary64_range() {
        let mut state = 0x1234_5678_9abc_def0_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            // Preserve random signs, fractions, subnormals, and finite exponents.
            let bits = if state & (0x7ff_u64 << 52) == 0x7ff_u64 << 52 {
                state ^ (1_u64 << 52)
            } else {
                state
            };
            f64::from_bits(bits)
        };
        for _ in 0..10_000 {
            let (a, b, c) = (next(), next(), next());
            assert_eq!(dot([a, 0.0, 0.0], [b, 0.0, 0.0]), a * b, "{a:e} * {b:e}");
            assert_eq!(
                dot([a, c, 0.0], [b, 1.0, 0.0]),
                a.mul_add(b, c),
                "{a:e} * {b:e} + {c:e}"
            );
        }
    }

    #[test]
    fn three_product_sums_match_an_independent_integer_oracle() {
        let mut state = 42_u64;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            i128::from(state & ((1_u64 << 40) - 1)) - (1_i128 << 39)
        };
        for _ in 0..1000 {
            let left = [next(), next(), next()];
            let right = [next(), next(), next()];
            let expected = left
                .into_iter()
                .zip(right)
                .map(|(a, b)| a * b)
                .sum::<i128>();
            for exponent in [-500, 0, 500] {
                // Integers fit exactly in binary64. Scaling by a binary power
                // keeps all inputs and final outputs in the normal range, so
                // only conversion of the exact integer sum rounds the oracle.
                let scale = 2.0_f64.powi(exponent);
                let a = left.map(|value| value as f64 * scale);
                let b = right.map(|value| value as f64);
                assert_eq!(dot(a, b), expected as f64 * scale);
                assert_eq!(dot(b, a), expected as f64 * scale);
            }
        }
    }

    #[test]
    fn rounds_subnormal_ties_and_accumulates_individually_underflowing_products() {
        let tiny = f64::from_bits(1);
        assert_eq!(dot([tiny; 3], [0.5; 3]), f64::from_bits(2));
        assert_eq!(dot([tiny, 0.0, 0.0], [0.5, 0.0, 0.0]), 0.0);
        assert_eq!(dot([tiny, tiny, 0.0], [0.5, 0.5, 0.0]), tiny);
        assert_eq!(dot([tiny; 3], [-0.5; 3]), -f64::from_bits(2));
        assert_eq!(
            dot([f64::MIN_POSITIVE, -tiny, 0.0], [1.0, 0.5, 0.0]),
            f64::MIN_POSITIVE
        );
    }

    #[test]
    fn rounds_normal_ties_and_reports_true_overflow() {
        let half_ulp = 2.0_f64.powi(-53);
        assert_eq!(dot([1.0, half_ulp, 0.0], [1.0; 3]), 1.0);
        assert_eq!(
            dot([1.0, half_ulp, f64::from_bits(1)], [1.0; 3]),
            f64::from_bits(1.0_f64.to_bits() + 1)
        );
        assert_eq!(
            dot(
                [f64::from_bits(1.0_f64.to_bits() + 1), half_ulp, 0.0],
                [1.0; 3]
            ),
            f64::from_bits(1.0_f64.to_bits() + 2)
        );
        assert_eq!(dot([f64::MAX; 3], [f64::MAX; 3]), f64::INFINITY);
        assert_eq!(dot([f64::MAX; 3], [-f64::MAX; 3]), f64::NEG_INFINITY);
        assert_eq!(dot([f64::MAX; 3], [1.0, 1.0, -1.0]), f64::MAX);
    }
}
