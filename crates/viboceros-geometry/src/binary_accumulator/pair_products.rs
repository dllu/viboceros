//! Exact product predicates shared by point and curve certificates.
use super::{add_product, decompose};
use std::cmp::Ordering;

// At most twenty-four products, including repeated products: 66 limbs at 2^-2148
// cover the entire finite binary64 range and all carries.
pub(crate) struct Products {
    positive: [u64; 66],
    negative: [u64; 66],
}
impl Default for Products {
    fn default() -> Self {
        Self {
            positive: [0; 66],
            negative: [0; 66],
        }
    }
}
impl Products {
    pub(crate) fn add(&mut self, a: f64, b: f64, subtract: bool) {
        let (a_bits, a_shift) = decompose(a);
        let (b_bits, b_shift) = decompose(b);
        let target = if a.is_sign_negative() ^ b.is_sign_negative() ^ subtract {
            &mut self.negative
        } else {
            &mut self.positive
        };
        add_product(
            target,
            u128::from(a_bits) * u128::from(b_bits),
            a_shift + b_shift,
        );
    }
    pub(crate) fn order(&self) -> Ordering {
        self.positive.iter().rev().cmp(self.negative.iter().rev())
    }
    pub(crate) fn compare_square(&self, distance: f64) -> Ordering {
        let mut negative = self.negative;
        let (bits, shift) = decompose(distance);
        add_product(
            &mut negative,
            u128::from(bits) * u128::from(bits),
            2 * shift,
        );
        self.positive.iter().rev().cmp(negative.iter().rev())
    }
}
