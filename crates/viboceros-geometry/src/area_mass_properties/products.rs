//! Exact, allocation-free hot-loop sums of finite binary64 products.
use super::*;
use crate::binary_accumulator::{add_product, decompose};
use num_bigint::{BigInt, Sign};

// Products occupy 4196 bits at quantum 2^-2148; 64 additional carry
// bits cover usize::MAX accumulated terms on supported targets.
const LIMBS: usize = 67;
const _: () = assert!(usize::BITS <= 64);

pub(super) struct Products {
    positive: [u64; LIMBS],
    negative: [u64; LIMBS],
    count: usize,
}

impl Default for Products {
    fn default() -> Self {
        Self {
            positive: [0; LIMBS],
            negative: [0; LIMBS],
            count: 0,
        }
    }
}

impl Products {
    pub(super) fn add(&mut self, a: Real, b: Real) -> Result<(), GeometryError> {
        require_finite([a, b], "area moment product")?;
        let count = self
            .count
            .checked_add(1)
            .ok_or(GeometryError::NumericalAccumulationCapacityExceeded)?;
        let (a_bits, a_shift) = decompose(a);
        let (b_bits, b_shift) = decompose(b);
        let target = if a.is_sign_negative() != b.is_sign_negative() {
            &mut self.negative
        } else {
            &mut self.positive
        };
        add_product(
            target,
            u128::from(a_bits) * u128::from(b_bits),
            a_shift + b_shift,
        );
        self.count = count;
        Ok(())
    }

    pub(super) fn total(&self) -> Rational {
        let integer = |words: &[u64; LIMBS]| {
            let digits: [u32; LIMBS * 2] =
                std::array::from_fn(|i| (words[i / 2] >> (32 * (i % 2))) as u32);
            BigInt::from_slice(Sign::Plus, &digits)
        };
        Rational::new(
            integer(&self.positive) - integer(&self.negative),
            BigInt::from(1) << 2148,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrounded_products_match_independent_rational_accumulation_across_binary64() {
        let mut sum = Products::default();
        let mut expected = Rational::zero();
        let mut state = 0x45217839331_u64;
        for _ in 0..2000 {
            let mut next = || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let bits = if state & (0x7ff_u64 << 52) == 0x7ff_u64 << 52 {
                    state ^ (1_u64 << 52)
                } else {
                    state
                };
                Real::from_bits(bits)
            };
            let (a, b) = (next(), next());
            sum.add(a, b).unwrap();
            expected += rational(a) * rational(b);
        }
        assert_eq!(sum.total(), expected);
        let before = sum.total();
        assert!(sum.add(Real::NAN, 1.).is_err());
        assert_eq!(sum.total(), before);
    }
}
