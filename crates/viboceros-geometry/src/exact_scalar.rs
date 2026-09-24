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

/// Evaluates `value * scale / divisor` with one final binary64 rounding.
/// Intermediate multiplication or division may exceed the binary64 range.
pub fn scaled_quotient(value: Real, scale: Real, divisor: Real) -> Result<Real, GeometryError> {
    require_finite([value, scale, divisor], "scaled quotient")?;
    if divisor == 0. {
        return Err(GeometryError::Degenerate {
            context: "scaled quotient divisor",
        });
    }
    if value == 0. || scale == 0. {
        return Ok(0.);
    }
    let exact = rational(value) * rational(scale) / rational(divisor);
    let result = scalar(&exact)?;
    if result == 0. {
        return Err(GeometryError::Degenerate {
            context: "scaled quotient underflow",
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_quotient_keeps_finite_results_across_intermediate_range_loss() {
        for (value, scale, divisor, expected) in [
            (2., 1e-3, 1e-308, 2e305),
            (2., 1e-300, 1e-308, 2e8),
            (2., 1e-308, 1e-308, 2.),
            (2., f64::from_bits(1), f64::from_bits(1), 2.),
            (2., 1e300, 1e300, 2.),
            (-2., 1e-3, 1e-308, -2e305),
            (2., 1e-3, -1e-308, -2e305),
        ] {
            let actual = scaled_quotient(value, scale, divisor).unwrap();
            assert!(
                (actual / expected - 1.).abs() < 1e-14,
                "{actual} != {expected}"
            );
        }
        assert!(scaled_quotient(2., 1e300, 1e-300).is_err());
        assert!(scaled_quotient(2., 1e-300, 1e300).is_err());
        assert!(scaled_quotient(2., 1., 0.).is_err());
        assert!(scaled_quotient(f64::NAN, 1., 2.).is_err());
        assert_eq!(scaled_quotient(0., 1., 2.).unwrap(), 0.);
    }
}
