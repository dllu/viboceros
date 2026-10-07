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

/// Maps a finite scalar affinely between two finite endpoint intervals, with
/// one final binary64 rounding. Extrapolation and reversed intervals are
/// supported. The source endpoints must differ. Intermediate interval widths,
/// products or ratios may exceed the binary64 range without losing a finite
/// result; a genuinely nonfinite result remains an error.
pub fn remap_scalar(
    value: Real,
    source: [Real; 2],
    target: [Real; 2],
) -> Result<Real, GeometryError> {
    require_finite(
        source.into_iter().chain(target).chain([value]),
        "scalar interval mapping",
    )?;
    if source[0] == source[1] {
        return Err(GeometryError::InvalidInterpolationParameter);
    }
    if value == source[0] {
        return Ok(target[0]);
    }
    if value == source[1] {
        return Ok(target[1]);
    }
    let a = rational(source[0]);
    let b = rational(source[1]);
    let t = rational(value);
    scalar(&((rational(target[0]) * (&b - &t) + rational(target[1]) * (&t - &a)) / (b - a)))
}

/// Linearly interpolate finite values at a parameter between two distinct
/// finite endpoint parameters. The endpoint order may be reversed.
///
/// Ordinary inputs use floating-point weights. Severe cancellation, subnormal
/// arithmetic, and overflowing intermediate ranges use exact rational weights
/// with one final rounding, retaining small coordinates on very long segments.
pub fn interpolate_scalar(
    values: [Real; 2],
    parameters: [Real; 2],
    parameter: Real,
) -> Result<Real, GeometryError> {
    require_finite(
        values.into_iter().chain(parameters).chain([parameter]),
        "scalar interpolation",
    )?;
    let [a, b] = parameters;
    if a == b || !(a.min(b)..=a.max(b)).contains(&parameter) {
        return Err(GeometryError::InvalidInterpolationParameter);
    }
    if parameter == a {
        return Ok(values[0]);
    }
    if parameter == b {
        return Ok(values[1]);
    }
    if values[0] == values[1] {
        return Ok(values[0]);
    }
    let span = b - a;
    let distances = [b - parameter, parameter - a];
    if span.is_normal() && distances.into_iter().all(Real::is_normal) {
        // Compute each weight independently; forming 1 - t loses a small
        // endpoint contribution when t rounds to one.
        let weights = distances.map(|distance| distance / span);
        let terms = std::array::from_fn::<_, 2, _>(|i| values[i] * weights[i]);
        let result = values[0].mul_add(weights[0], terms[1]);
        if result.is_normal()
            && weights.into_iter().all(Real::is_normal)
            && (0..2).all(|i| values[i] == 0. || terms[i].is_normal())
            && result.abs() >= terms[0].abs().max(terms[1].abs()) * 0.125
            && (values[0].min(values[1])..=values[0].max(values[1])).contains(&result)
        {
            return Ok(result);
        }
    }
    let a = rational(a);
    let b = rational(b);
    let parameter = rational(parameter);
    scalar(
        &((rational(values[0]) * (&b - &parameter) + rational(values[1]) * (&parameter - &a))
            / (b - a)),
    )
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
    fn interval_mapping_retains_extrapolation_reversal_and_extreme_ranges() {
        for (value, source, target, expected) in [
            (2., [0., 1.], [3., 5.], 7.),
            (0.25, [1., 0.], [2., 6.], 5.),
            (0., [-Real::MAX, Real::MAX], [0., 1.], 0.5),
            (400., [0., 1e24], [0., 1e24], 400.),
            (Real::from_bits(1), [0., Real::from_bits(2)], [0., 1.], 0.5),
            (1., [0., 1.], [-Real::MAX, Real::MAX], Real::MAX),
        ] {
            assert_eq!(remap_scalar(value, source, target).unwrap(), expected);
        }
        assert!(remap_scalar(0., [1., 1.], [0., 1.]).is_err());
        assert!(remap_scalar(Real::NAN, [0., 1.], [0., 1.]).is_err());
        assert!(remap_scalar(2., [0., 1.], [0., Real::MAX]).is_err());
    }

    #[test]
    fn scalar_interpolation_retains_small_coordinates_and_extreme_ranges() {
        for scale in [1e24, 1e200, Real::MAX] {
            for value in [-12., 0., 4., Real::from_bits(1)] {
                assert_eq!(
                    interpolate_scalar([-scale, scale], [-scale, scale], value).unwrap(),
                    value
                );
                assert_eq!(
                    interpolate_scalar([scale, -scale], [scale, -scale], value).unwrap(),
                    value
                );
            }
        }
        let tiny = Real::from_bits(1);
        assert_eq!(
            interpolate_scalar([tiny, tiny * 2.], [0., tiny * 2.], tiny).unwrap(),
            tiny * 2.
        );
        assert_eq!(
            interpolate_scalar([0., 1e24], [0., 1e24], 400.).unwrap(),
            400.
        );
        assert_eq!(
            interpolate_scalar([Real::MAX, Real::MAX], [-Real::MAX, Real::MAX], 0.).unwrap(),
            Real::MAX
        );
        for (values, parameters, parameter) in [
            ([0., Real::NAN], [0., 1.], 0.5),
            ([0., 1.], [0., Real::INFINITY], 0.5),
            ([0., 1.], [0., 1.], Real::NAN),
            ([0., 1.], [1., 1.], 1.),
            ([0., 1.], [0., 1.], 2.),
        ] {
            assert!(interpolate_scalar(values, parameters, parameter).is_err());
        }
    }

    #[test]
    fn scalar_interpolation_matches_rational_reference_across_finite_range() {
        let mut state = 73_u64;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let mut bits = state;
            if bits & (0x7ff << 52) == 0x7ff << 52 {
                bits ^= 1 << 52;
            }
            Real::from_bits(bits)
        };
        for _ in 0..1024 {
            let values = [next(), next()];
            let parameters = [next(), next()];
            if parameters[0] == parameters[1] {
                continue;
            }
            let parameter = parameters[0].midpoint(parameters[1]);
            let t = (rational(parameter) - rational(parameters[0]))
                / (rational(parameters[1]) - rational(parameters[0]));
            let reference =
                scalar(&(rational(values[0]) + t * (rational(values[1]) - rational(values[0]))))
                    .unwrap();
            let actual = interpolate_scalar(values, parameters, parameter).unwrap();
            assert!(
                (actual - reference).abs() <= reference.abs() * 16. * Real::EPSILON
                    || actual == reference,
                "{values:?} {parameters:?} {parameter}: {actual} vs {reference}"
            );
        }
    }

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
