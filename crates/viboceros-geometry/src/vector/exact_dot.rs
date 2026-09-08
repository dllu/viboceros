//! Allocation-free fallback for dot products outside ordinary floating-point range.
//!
//! Every finite binary64 value is an integer significand times a power of two.
//! Products therefore fit an integer accumulator with quantum 2^-2148. Three
//! products need at most 4198 bits, including carry; 66 limbs provide 4224.
//! Separate positive/negative magnitudes avoid losing small terms before large
//! terms cancel. Only the final conversion rounds (nearest, ties to even).

const LIMBS: usize = 66;
type Magnitude = [u64; LIMBS];

pub(super) fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
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
    let negative_result = positive.iter().rev().cmp(negative.iter().rev()).is_lt();
    let (mut magnitude, subtract) = if negative_result {
        (negative, positive)
    } else {
        (positive, negative)
    };
    let mut borrow = false;
    for (word, subtract_word) in magnitude.iter_mut().zip(subtract) {
        let (difference, first_borrow) = word.overflowing_sub(subtract_word);
        let (difference, second_borrow) = difference.overflowing_sub(u64::from(borrow));
        *word = difference;
        borrow = first_borrow || second_borrow;
    }
    debug_assert!(!borrow);
    let value = rounded(&magnitude);
    if negative_result { -value } else { value }
}

// Return the unsigned significand and its shift relative to 2^-1074.
fn decompose(value: f64) -> (u64, usize) {
    debug_assert!(value.is_finite());
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as usize;
    let fraction = bits & ((1_u64 << 52) - 1);
    if exponent == 0 {
        (fraction, 0)
    } else {
        (fraction | (1_u64 << 52), exponent - 1)
    }
}

fn add_product(target: &mut Magnitude, mut product: u128, shift: usize) {
    let mut index = shift / 64;
    let offset = shift % 64;
    while product != 0 {
        let word = product as u64;
        add_word(target, index, word << offset);
        if offset != 0 {
            add_word(target, index + 1, word >> (64 - offset));
        }
        product >>= 64;
        index += 1;
    }
}

fn add_word(target: &mut Magnitude, mut index: usize, mut word: u64) {
    while word != 0 {
        let (sum, carry) = target[index].overflowing_add(word);
        target[index] = sum;
        word = u64::from(carry);
        index += 1;
    }
}

fn rounded(magnitude: &Magnitude) -> f64 {
    let Some(index) = magnitude.iter().rposition(|word| *word != 0) else {
        return 0.0;
    };
    let highest = index * 64 + 63 - magnitude[index].leading_zeros() as usize;
    // Keep at most 53 significant bits, but never use a quantum finer than
    // the smallest subnormal binary64 value (accumulator bit 1074).
    let mut shift = highest.saturating_sub(52).max(1074);
    let index = shift / 64;
    let offset = shift % 64;
    let mut significand = magnitude[index] >> offset;
    if offset != 0 && index + 1 < LIMBS {
        significand |= magnitude[index + 1] << (64 - offset);
    }
    let guard_index = (shift - 1) / 64;
    let guard_offset = (shift - 1) % 64;
    let guard = magnitude[guard_index] & (1_u64 << guard_offset) != 0;
    let sticky = magnitude[..guard_index].iter().any(|word| *word != 0)
        || magnitude[guard_index] & ((1_u64 << guard_offset) - 1) != 0;
    if guard && (sticky || significand & 1 != 0) {
        significand += 1;
    }
    if significand < 1_u64 << 52 {
        return f64::from_bits(significand);
    }
    if significand == 1_u64 << 53 {
        significand >>= 1;
        shift += 1;
    }
    // The normal exponent is shift - 2148 + 52; add the binary64 bias.
    let biased_exponent = shift - 1073;
    if biased_exponent >= 0x7ff {
        return f64::INFINITY;
    }
    f64::from_bits((biased_exponent as u64) << 52 | (significand & ((1_u64 << 52) - 1)))
}

#[cfg(test)]
mod tests {
    use super::dot;

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
