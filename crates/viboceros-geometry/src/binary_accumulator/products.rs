//! Conservative selection of a compact window for binary64 product sums.
use super::decompose;

#[cfg(test)]
mod tests;

/// All inputs are finite. A pair contributes at most 106 bits; scaled pairs
/// contribute at most 159. Extra exponent shifts are exact, not rescalings.
pub(crate) fn compact_base<const N: usize>(
    left: &[f64; N],
    right: &[f64; N],
    extra_shift: usize,
    product_bits: usize,
) -> Option<usize> {
    let mut low = usize::MAX;
    let mut high = 0;
    let carry_bits = (usize::BITS - N.saturating_sub(1).leading_zeros()) as usize;
    for (&a, &b) in left.iter().zip(right) {
        if a == 0. || b == 0. {
            continue;
        }
        let shift = decompose(a).1 + decompose(b).1 + extra_shift;
        low = low.min(shift);
        high = high.max(shift + product_bits);
        // The interval can only expand as further terms are considered.
        // Bound same-sign totals, not the result after cancellation.
        if high + carry_bits - low / 64 * 64 > 256 {
            return None;
        }
    }
    Some(if low == usize::MAX { 0 } else { low / 64 * 64 })
}
