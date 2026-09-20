//! Fixed-limb exact binary accumulation shared by sums and dot products.
//! Full-range QUANTUM is the negated exponent of bit zero (at least 1074).
//! Compact windows carry their own exact binary exponent origin.

pub(crate) mod products;
#[cfg(test)]
mod tests;

pub(crate) fn finish<const N: usize, const QUANTUM: usize>(
    positive: [u64; N],
    negative: [u64; N],
) -> f64 {
    finish_at(positive, negative, -(QUANTUM as i32))
}

/// Round a finite integer accumulator whose bit zero represents 2^origin.
/// Callers derive origin from bounded binary64 product exponents.
#[inline]
pub(crate) fn finish_at<const N: usize>(
    positive: [u64; N],
    negative: [u64; N],
    origin: i32,
) -> f64 {
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
    let value = rounded(&magnitude, origin);
    if negative_result { -value } else { value }
}

// Return the unsigned significand and its shift relative to 2^-1074.
pub(crate) fn decompose(value: f64) -> (u64, usize) {
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

pub(crate) fn add_product<const N: usize>(target: &mut [u64; N], mut product: u128, shift: usize) {
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

fn add_word<const N: usize>(target: &mut [u64; N], mut index: usize, mut word: u64) {
    while word != 0 {
        let (sum, carry) = target[index].overflowing_add(word);
        target[index] = sum;
        word = u64::from(carry);
        index += 1;
    }
}

fn rounded<const N: usize>(magnitude: &[u64; N], origin: i32) -> f64 {
    let Some(index) = magnitude.iter().rposition(|word| *word != 0) else {
        return 0.0;
    };
    let highest = index * 64 + 63 - magnitude[index].leading_zeros() as usize;
    // Entirely below half the smallest subnormal. Keep the result unsigned;
    // finish_at restores a negative exact sum's negative rounded zero.
    if highest as i32 + origin < -1075 {
        return 0.0;
    }
    let shift = highest
        .saturating_sub(52)
        .max((-1074 - origin).max(0) as usize);
    let index = shift / 64;
    let offset = shift % 64;
    let mut significand = magnitude.get(index).copied().unwrap_or(0) >> offset;
    if offset != 0
        && let Some(next) = magnitude.get(index + 1)
    {
        significand |= next << (64 - offset);
    }
    if shift != 0 {
        let guard_index = (shift - 1) / 64;
        let guard_offset = (shift - 1) % 64;
        let guard_word = magnitude.get(guard_index).copied().unwrap_or(0);
        let guard = guard_word & (1_u64 << guard_offset) != 0;
        let sticky = magnitude.iter().take(guard_index).any(|word| *word != 0)
            || guard_word & ((1_u64 << guard_offset) - 1) != 0;
        if guard && (sticky || significand & 1 != 0) {
            significand += 1;
        }
    }
    if significand == 0 {
        return 0.0;
    }
    let leading = 63 - significand.leading_zeros();
    let exponent = origin + shift as i32 + leading as i32;
    if exponent > 1023 {
        return f64::INFINITY;
    }
    if exponent < -1022 {
        let subnormal_shift = origin + shift as i32 + 1074;
        debug_assert!((0..52).contains(&subnormal_shift));
        return f64::from_bits(significand << subnormal_shift);
    }
    // Compact windows can begin above the subnormal quantum. Their final
    // integer may have fewer than 53 bits; normalize it without rounding again.
    let significand = if leading > 52 {
        significand >> (leading - 52)
    } else {
        significand << (52 - leading)
    };
    f64::from_bits(((exponent + 1023) as u64) << 52 | (significand & ((1_u64 << 52) - 1)))
}
