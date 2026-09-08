//! Allocation-free, correctly rounded sums of finite binary64 values.

use crate::{
    GeometryError, Real,
    binary_accumulator::{add_product, decompose, finish},
    require_finite,
};

// One binary64 term needs 2098 bits at quantum 2^-1074. Space for another
// 64 carry bits covers usize::MAX terms on all supported 32/64-bit targets.
const LIMBS: usize = 34;
const _: () = assert!(usize::BITS <= 64);

/// Exact accumulation of finite terms with one final nearest-even rounding.
/// Intermediate overflow and cancellation do not discard small contributions.
#[derive(Clone, Debug)]
pub struct FiniteSum {
    positive: [u64; LIMBS],
    negative: [u64; LIMBS],
    count: usize,
}

impl Default for FiniteSum {
    fn default() -> Self {
        Self {
            positive: [0; LIMBS],
            negative: [0; LIMBS],
            count: 0,
        }
    }
}

impl FiniteSum {
    /// Adds one finite term. An invalid term leaves the accumulator unchanged.
    pub fn add(&mut self, value: Real) -> Result<(), GeometryError> {
        require_finite([value], "sum term")?;
        let count = self
            .count
            .checked_add(1)
            .ok_or(GeometryError::NumericalAccumulationCapacityExceeded)?;
        let (significand, shift) = decompose(value);
        let target = if value.is_sign_negative() {
            &mut self.negative
        } else {
            &mut self.positive
        };
        add_product(target, u128::from(significand), shift);
        self.count = count;
        Ok(())
    }

    /// Returns the total, rejecting a non-finite rounded result. Exact cancellation
    /// and an empty sum return positive zero; all finite subnormals are retained.
    pub fn total(&self) -> Result<Real, GeometryError> {
        let value = finish::<LIMBS, 1074>(self.positive, self.negative);
        require_finite([value], "sum total")?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sum(values: impl IntoIterator<Item = f64>) -> Result<f64, GeometryError> {
        let mut sum = FiniteSum::default();
        for value in values {
            sum.add(value)?;
        }
        sum.total()
    }

    #[test]
    fn pair_sums_match_hardware_rounding_across_binary64_range() {
        let mut state = 0x0023_90ab_cdef_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let bits = if state & (0x7ff_u64 << 52) == 0x7ff_u64 << 52 {
                state ^ (1_u64 << 52)
            } else {
                state
            };
            f64::from_bits(bits)
        };
        for _ in 0..10_000 {
            let (a, b) = (next(), next());
            let expected = a + b;
            if expected.is_finite() {
                assert_eq!(sum([a, b]).unwrap(), expected, "{a:e} + {b:e}");
            } else {
                assert!(sum([a, b]).is_err());
            }
        }
    }

    #[test]
    fn cancellation_preserves_subnormals_after_overflowing_prefixes() {
        let tiny = f64::from_bits(1);
        for (values, expected) in [
            ([f64::MAX, f64::MAX, -f64::MAX, 0., 0.], f64::MAX),
            ([f64::MAX, f64::MAX, tiny, -f64::MAX, -f64::MAX], tiny),
            ([-f64::MAX, -f64::MAX, -tiny, f64::MAX, f64::MAX], -tiny),
            (
                [f64::MIN_POSITIVE, -tiny, 0., 0., 0.],
                f64::from_bits((1_u64 << 52) - 1),
            ),
        ] {
            assert_eq!(sum(values).unwrap(), expected);
            assert_eq!(sum(values.into_iter().rev()).unwrap(), expected);
        }
        assert_eq!(sum([]).unwrap().to_bits(), 0);
        assert_eq!(sum([-0., 0.]).unwrap().to_bits(), 0);
        assert!(sum([f64::MAX; 2]).is_err());
    }

    #[test]
    fn one_final_rounding_uses_nearest_even() {
        let half_ulp = 2_f64.powi(-53);
        assert_eq!(sum([1., half_ulp]).unwrap(), 1.);
        assert_eq!(
            sum([1., half_ulp, f64::from_bits(1)]).unwrap(),
            f64::from_bits(1_f64.to_bits() + 1)
        );
        assert_eq!(
            sum([f64::from_bits(1_f64.to_bits() + 1), half_ulp]).unwrap(),
            f64::from_bits(1_f64.to_bits() + 2)
        );
    }

    #[test]
    fn sums_match_independent_integer_totals_across_scales() {
        let mut state = 91_u64;
        for _ in 0..100 {
            let values: Vec<i128> = (0..127)
                .map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    i128::from(state >> 12) - (1_i128 << 51)
                })
                .collect();
            let expected: i128 = values.iter().sum();
            for exponent in [-900, 0, 900] {
                let scale = 2_f64.powi(exponent);
                assert_eq!(
                    sum(values.iter().map(|&value| value as f64 * scale)).unwrap(),
                    expected as f64 * scale
                );
                assert_eq!(
                    sum(values.iter().rev().map(|&value| value as f64 * scale)).unwrap(),
                    expected as f64 * scale
                );
            }
        }
    }

    #[test]
    fn rejected_terms_and_total_queries_do_not_corrupt_the_accumulator() {
        let mut sum = FiniteSum::default();
        sum.add(f64::MAX).unwrap();
        sum.add(f64::MAX).unwrap();
        assert!(sum.total().is_err());
        sum.add(-f64::MAX).unwrap();
        assert_eq!(sum.total().unwrap(), f64::MAX);
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(sum.add(invalid).is_err());
            assert_eq!(sum.total().unwrap(), f64::MAX);
        }
        sum.count = usize::MAX;
        assert!(matches!(
            sum.add(1.),
            Err(GeometryError::NumericalAccumulationCapacityExceeded)
        ));
        assert_eq!(sum.total().unwrap(), f64::MAX);
    }
}
