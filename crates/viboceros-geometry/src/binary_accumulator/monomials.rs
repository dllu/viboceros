//! Allocation-free exact sums of three/four-factor binary64 monomials.
use super::{add_product, decompose};
use crate::exact_scalar::Rational;
use crate::{GeometryError, Real, require_finite};
use num_bigint::{BigInt, Sign};

pub(crate) type Cubics = Monomials<3, 100>;
pub(crate) type Quartics = Monomials<4, 133>;

pub(crate) struct Monomials<const DEGREE: usize, const LIMBS: usize> {
    positive: [u64; LIMBS],
    negative: [u64; LIMBS],
    count: usize,
}

impl<const D: usize, const L: usize> Default for Monomials<D, L> {
    fn default() -> Self {
        // Each finite factor occupies at most 2098 bits at quantum 2^-1074.
        // Additional carry bits cover every representable term count.
        assert!(D > 0 && D <= 4 && L * 64 >= D * 2098 + usize::BITS as usize);
        Self {
            positive: [0; L],
            negative: [0; L],
            count: 0,
        }
    }
}

impl<const D: usize, const L: usize> Monomials<D, L> {
    pub(crate) fn add(&mut self, factors: [Real; D]) -> Result<(), GeometryError> {
        require_finite(factors, "mass monomial")?;
        let count = self
            .count
            .checked_add(1)
            .ok_or(GeometryError::NumericalAccumulationCapacityExceeded)?;
        if factors.contains(&0.) {
            self.count = count;
            return Ok(());
        }
        let mut words = [1_u64, 0, 0, 0];
        let mut shift = 0;
        let mut negative = false;
        for factor in factors {
            let (bits, exponent) = decompose(factor);
            shift += exponent;
            negative ^= factor.is_sign_negative();
            let mut carry = 0_u128;
            for word in &mut words {
                let product = u128::from(*word) * u128::from(bits) + carry;
                *word = product as u64;
                carry = product >> 64;
            }
            debug_assert_eq!(carry, 0);
        }
        let target = if negative {
            &mut self.negative
        } else {
            &mut self.positive
        };
        for (i, word) in words.into_iter().enumerate() {
            add_product(target, u128::from(word), shift + 64 * i);
        }
        self.count = count;
        Ok(())
    }

    pub(crate) fn total(&self) -> Rational {
        let integer = |words: &[u64; L]| {
            let digits = words
                .iter()
                .flat_map(|w| [*w as u32, (w >> 32) as u32])
                .collect::<Vec<_>>();
            BigInt::from_slice(Sign::Plus, &digits)
        };
        Rational::new(
            integer(&self.positive) - integer(&self.negative),
            BigInt::from(1) << (D * 1074),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exact_scalar::rational;
    use num_traits::Zero;

    #[test]
    fn cubic_and_quartic_sums_match_independent_full_range_rationals() {
        let mut state = 0x541ff9814_u64;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let bits = if state & (0x7ff_u64 << 52) == 0x7ff_u64 << 52 {
                state ^ (1_u64 << 52)
            } else {
                state
            };
            Real::from_bits(bits)
        };
        let (mut a, mut b) = (Cubics::default(), Quartics::default());
        let (mut x, mut y) = (Rational::zero(), Rational::zero());
        for _ in 0..200 {
            let f = [next(), next(), next(), next()];
            a.add([f[0], f[1], f[2]]).unwrap();
            b.add(f).unwrap();
            let p = rational(f[0]) * rational(f[1]) * rational(f[2]);
            x += &p;
            y += p * rational(f[3]);
        }
        assert_eq!(a.total(), x);
        assert_eq!(b.total(), y);
        assert!(b.add([1., 2., 3., Real::INFINITY]).is_err());
        assert_eq!(b.total(), y);
    }
}
