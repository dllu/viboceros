//! Exact conversion of validated finite binary64 values and one final rounding.
use crate::{GeometryError, Real, require_finite};
pub(crate) use num_rational::BigRational as Rational;
use num_traits::ToPrimitive;

pub(crate) fn rational(value: Real) -> Rational {
    Rational::from_float(value).expect("validated finite geometry scalar")
}

pub(crate) fn scalar(value: &Rational) -> Result<Real, GeometryError> {
    let value = value.to_f64().ok_or(GeometryError::NonFinite {
        context: "exact rational projection",
    })?;
    require_finite([value], "exact rational projection")?;
    Ok(value)
}
