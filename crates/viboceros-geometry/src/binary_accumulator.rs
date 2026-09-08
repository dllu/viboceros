//! Fixed-limb exact binary accumulation shared by sums and dot products.
//! QUANTUM is the negated exponent of accumulator bit zero (at least 1074).

pub(crate) fn finish<const N: usize, const QUANTUM: usize>(
    positive: [u64; N],
    negative: [u64; N],
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
    let value = rounded::<N, QUANTUM>(&magnitude);
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

fn rounded<const N: usize, const QUANTUM: usize>(magnitude: &[u64; N]) -> f64 {
    let Some(index) = magnitude.iter().rposition(|word| *word != 0) else {
        return 0.0;
    };
    let highest = index * 64 + 63 - magnitude[index].leading_zeros() as usize;
    // Keep at most 53 significant bits, but never use a quantum finer than
    // the smallest subnormal binary64 value.
    let mut shift = highest.saturating_sub(52).max(QUANTUM - 1074);
    let index = shift / 64;
    let offset = shift % 64;
    let mut significand = magnitude[index] >> offset;
    if offset != 0 && index + 1 < N {
        significand |= magnitude[index + 1] << (64 - offset);
    }
    if shift != 0 {
        let guard_index = (shift - 1) / 64;
        let guard_offset = (shift - 1) % 64;
        let guard = magnitude[guard_index] & (1_u64 << guard_offset) != 0;
        let sticky = magnitude[..guard_index].iter().any(|word| *word != 0)
            || magnitude[guard_index] & ((1_u64 << guard_offset) - 1) != 0;
        if guard && (sticky || significand & 1 != 0) {
            significand += 1;
        }
    }
    if significand < 1_u64 << 52 {
        return f64::from_bits(significand);
    }
    if significand == 1_u64 << 53 {
        significand >>= 1;
        shift += 1;
    }
    // The normal exponent is shift - QUANTUM + 52; add bias 1023.
    let biased_exponent = shift + 1075 - QUANTUM;
    if biased_exponent >= 0x7ff {
        return f64::INFINITY;
    }
    f64::from_bits((biased_exponent as u64) << 52 | (significand & ((1_u64 << 52) - 1)))
}
