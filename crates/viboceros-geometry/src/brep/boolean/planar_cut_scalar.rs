//! Scaled scalar arithmetic shared by planar boundary cut coefficients.
use super::*;

pub(super) fn square_root(value: &Rational) -> Result<f64, GeometryError> {
    if value.is_zero() {
        return Ok(0.);
    }
    // Normalize by an even power so neither the square nor its scale needs
    // to fit binary64. Only the normalized sqrt and final root are rounded.
    let exponent = value.numer().bits() as i64 - value.denom().bits() as i64;
    let even = exponent.div_euclid(2) * 2;
    let power = if even >= 0 {
        Rational::from_integer(num_bigint::BigInt::from(1u8) << even as usize)
    } else {
        Rational::new(
            num_bigint::BigInt::from(1u8),
            num_bigint::BigInt::from(1u8) << (-even) as usize,
        )
    };
    let normalized = scalar(&(value / &power))?.sqrt();
    let result = scalar(
        &(rational(normalized)
            * if even / 2 >= 0 {
                Rational::from_integer(num_bigint::BigInt::from(1u8) << (even / 2) as usize)
            } else {
                Rational::new(
                    num_bigint::BigInt::from(1u8),
                    num_bigint::BigInt::from(1u8) << (-even / 2) as usize,
                )
            }),
    )?;
    if result == 0. {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    Ok(result)
}
