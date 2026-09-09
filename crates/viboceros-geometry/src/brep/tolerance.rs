//! Outward-rounded propagation of validated B-rep component tolerances.
use super::{GeometryError, Real, require_nonnegative_finite};

pub(super) fn scaled_tolerance(value: Real, scale: Real) -> Result<Real, GeometryError> {
    if value == 0.0 {
        return Ok(0.0);
    }
    let scaled = value * scale;
    require_nonnegative_finite(scaled, "transformed B-rep component tolerance")?;
    if product_rounded_down(value, scale, scaled) {
        let upper = scaled.next_up();
        require_nonnegative_finite(upper, "transformed B-rep component tolerance")?;
        return Ok(upper);
    }
    Ok(scaled)
}

// Compare a finite nonnegative product with its rounded value using integer
// significands. FMA alone cannot detect residuals smaller than a subnormal.
fn product_rounded_down(a: Real, b: Real, rounded: Real) -> bool {
    use crate::binary_accumulator::decompose;
    let (a_bits, a_shift) = decompose(a);
    let (b_bits, b_shift) = decompose(b);
    let product = u128::from(a_bits) * u128::from(b_bits);
    if product == 0 {
        return false;
    }
    if rounded == 0. {
        return true;
    }
    let (rounded_bits, rounded_shift) = decompose(rounded);
    let rounded_bits = u128::from(rounded_bits);
    let product_width = 128 - product.leading_zeros();
    let rounded_width = 128 - rounded_bits.leading_zeros();
    // Product quantum is 2^-2148; a single value has quantum 2^-1074.
    let product_top = a_shift + b_shift + product_width as usize;
    let rounded_top = rounded_shift + 1074 + rounded_width as usize;
    match product_top.cmp(&rounded_top) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => {
            if product_width >= rounded_width {
                product > (rounded_bits << (product_width - rounded_width))
            } else {
                (product << (rounded_width - product_width)) > rounded_bits
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transformed_component_tolerances_round_outward_without_inflating_exact_products() {
        let tiny = Real::from_bits(1);
        assert_eq!(scaled_tolerance(tiny, 0.5).unwrap(), tiny);
        assert_eq!(scaled_tolerance(tiny, 1.).unwrap(), tiny);
        assert_eq!(scaled_tolerance(Real::MAX * 0.5, 2.).unwrap(), Real::MAX);
        assert_eq!(scaled_tolerance(0., Real::MAX).unwrap(), 0.);
        assert_eq!(scaled_tolerance(1., 0.).unwrap(), 0.);
        let a = 1. + Real::EPSILON;
        // Exact square is 1 + 2*epsilon + epsilon^2, just above the rounded value.
        assert_eq!(
            scaled_tolerance(a, a).unwrap(),
            (1. + 2. * Real::EPSILON).next_up()
        );
        assert!(scaled_tolerance(Real::MAX, 2.).is_err());
    }

    #[test]
    fn tolerance_product_direction_matches_fused_residual_in_normal_range() {
        let mut state = 42_u64;
        for _ in 0..10_000 {
            let mut next = || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                Real::from_bits((1023_u64 << 52) | (state & ((1_u64 << 52) - 1)))
            };
            let (a, b) = (next(), next());
            for exponent in [-200, 0, 200] {
                let a = a * 2_f64.powi(exponent);
                let rounded = a * b;
                // All product residuals here are far above the subnormal
                // floor, so hardware FMA supplies an independent direction.
                let downward = a.mul_add(b, -rounded) > 0.;
                assert_eq!(product_rounded_down(a, b, rounded), downward);
                assert_eq!(
                    scaled_tolerance(a, b).unwrap(),
                    if downward { rounded.next_up() } else { rounded }
                );
            }
        }
    }
}
