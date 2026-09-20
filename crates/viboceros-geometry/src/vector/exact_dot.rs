//! Allocation-free fallback for dot products outside ordinary floating-point range.
//!
//! Every finite binary64 value is an integer significand times a power of two.
//! Products therefore fit an integer accumulator with quantum 2^-2148. Up to six
//! products need at most 4199 bits, including carry; 66 limbs provide 4224.
//! Separate positive/negative magnitudes avoid losing small terms before large
//! terms cancel. Only the final conversion rounds (nearest, ties to even).

use crate::binary_accumulator::{add_product, decompose, finish_at, products::compact_base};

/// Scale the exact sum before rounding. Triple products use quantum 2^-3222.
pub(super) fn scaled_dot<const N: usize>(left: [f64; N], right: [f64; N], scale: f64) -> f64 {
    assert!(N <= 6, "exact scaled dot accumulator capacity");
    let (c, c_shift) = decompose(scale);
    if c == 0 {
        return 0.;
    }
    if let Some(base) = compact_base(&left, &right, c_shift, 159) {
        scaled_accumulate::<N, 4>(left, right, c, c_shift, scale.is_sign_negative(), base)
    } else {
        scaled_accumulate::<N, 99>(left, right, c, c_shift, scale.is_sign_negative(), 0)
    }
}

#[inline]
fn scaled_accumulate<const N: usize, const WORDS: usize>(
    left: [f64; N],
    right: [f64; N],
    c: u64,
    c_shift: usize,
    negative_scale: bool,
    base: usize,
) -> f64 {
    let mut positive = [0; WORDS];
    let mut negative = [0; WORDS];
    for (a, b) in left.into_iter().zip(right) {
        if a == 0. || b == 0. {
            continue;
        }
        let (a_bits, a_shift) = decompose(a);
        let (b_bits, b_shift) = decompose(b);
        let product = u128::from(a_bits) * u128::from(b_bits);
        let shift = a_shift + b_shift + c_shift - base;
        let target = if a.is_sign_negative() ^ b.is_sign_negative() ^ negative_scale {
            &mut negative
        } else {
            &mut positive
        };
        // Both parts have the same sign and sum to one exact triple product.
        // N-product carry headroom therefore also bounds every partial sum.
        add_product(target, u128::from(product as u64) * u128::from(c), shift);
        add_product(target, (product >> 64) * u128::from(c), shift + 64);
    }
    finish_at(positive, negative, base as i32 - 3222)
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
    if let Some(base) = compact_base(&left, &right, 0, 106) {
        accumulate::<N, 4, QUANTUM>(left, right, base)
    } else {
        accumulate::<N, 66, QUANTUM>(left, right, 0)
    }
}

#[inline]
fn accumulate<const N: usize, const WORDS: usize, const QUANTUM: usize>(
    left: [f64; N],
    right: [f64; N],
    base: usize,
) -> f64 {
    let mut positive = [0; WORDS];
    let mut negative = [0; WORDS];
    for (a, b) in left.into_iter().zip(right) {
        if a == 0. || b == 0. {
            continue;
        }
        let (a_bits, a_shift) = decompose(a);
        let (b_bits, b_shift) = decompose(b);
        let target = if a.is_sign_negative() ^ b.is_sign_negative() {
            &mut negative
        } else {
            &mut positive
        };
        add_product(
            target,
            u128::from(a_bits) * u128::from(b_bits),
            a_shift + b_shift - base,
        );
    }
    finish_at(positive, negative, base as i32 - QUANTUM as i32)
}

#[cfg(test)]
mod tests {
    use super::dot;

    #[test]
    fn clustered_products_and_scales_match_independent_rational_rounding() {
        use num_rational::BigRational as R;
        use num_traits::{ToPrimitive, Zero};
        let mut state = 53_u64;
        for center in [-1000, -512, -1, 0, 512, 1000] {
            for case in 0..128 {
                let mut next = || {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let exponent = (1023 + center + (state % 9) as i32 - 4).clamp(0, 2046) as u64;
                    f64::from_bits(
                        (state & ((1_u64 << 63) | ((1_u64 << 52) - 1))) | (exponent << 52),
                    )
                };
                let mut left = std::array::from_fn::<_, 6, _>(|_| next());
                let mut right = std::array::from_fn::<_, 6, _>(|_| next());
                if case % 2 == 0 {
                    left[1] = left[0];
                    right[1] = -right[0];
                }
                let exact = left.into_iter().zip(right).fold(R::zero(), |sum, (a, b)| {
                    sum + R::from_float(a).unwrap() * R::from_float(b).unwrap()
                });
                assert_eq!(
                    dot(left, right).to_bits(),
                    exact.to_f64().unwrap().to_bits()
                );
                assert_eq!(
                    super::half_dot(left, right).to_bits(),
                    (&exact / R::from_integer(2.into()))
                        .to_f64()
                        .unwrap()
                        .to_bits()
                );
                let scale = match case % 4 {
                    0 => 0.,
                    1 => -1.,
                    2 => 2_f64.powi(-600),
                    _ => 2_f64.powi(600),
                };
                assert_eq!(
                    super::scaled_dot(left, right, scale).to_bits(),
                    (&exact * R::from_float(scale).unwrap())
                        .to_f64()
                        .unwrap()
                        .to_bits()
                );
            }
        }
    }

    #[test]
    fn scaled_dot_matches_independent_fraction_reference_bit_for_bit() {
        let mut count = 0;
        for line in include_str!("exact_dot/scaled_reference.txt")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields = line
                .split_whitespace()
                .map(|word| u64::from_str_radix(word, 16).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(fields.len(), 14);
            let left = std::array::from_fn::<_, 6, _>(|i| f64::from_bits(fields[i]));
            let right = std::array::from_fn::<_, 6, _>(|i| f64::from_bits(fields[i + 6]));
            let scale = f64::from_bits(fields[12]);
            assert_eq!(
                super::scaled_dot(left, right, scale).to_bits(),
                fields[13],
                "case {count}: {line}"
            );
            count += 1;
        }
        assert_eq!(count, 256);
    }

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
