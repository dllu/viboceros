use crate::{GeometryError, Real, require_finite};
use std::ops::RangeInclusive;

/// Error-free TwoDiff, declining nonfinite intermediates or a nonzero residual.
pub(crate) fn exact_difference(a: Real, b: Real) -> Option<Real> {
    let difference = a - b;
    let b_virtual = a - difference;
    let a_virtual = difference + b_virtual;
    let b_error = b_virtual - b;
    let a_error = a - a_virtual;
    let error = a_error + b_error;
    [difference, b_virtual, a_virtual, b_error, a_error, error]
        .into_iter()
        .all(Real::is_finite)
        .then_some(difference)
        .filter(|_| error == 0.)
}

/// Removes a same-sign domain's nearest endpoint only when every supplied
/// coordinate (including exterior knots) translates without any rounding.
pub(crate) fn lossless_parameter_origin(
    domain: RangeInclusive<Real>,
    mut coordinates: impl Iterator<Item = Real>,
) -> Real {
    let candidate = if *domain.start() > 0. {
        *domain.start()
    } else if *domain.end() < 0. {
        *domain.end()
    } else {
        return 0.;
    };
    if coordinates.all(|value| exact_difference(value, candidate).is_some()) {
        candidate
    } else {
        0.
    }
}

pub(crate) fn check_trim_interval(
    interval: &RangeInclusive<Real>,
    domain: RangeInclusive<Real>,
) -> Result<(), GeometryError> {
    let start = *interval.start();
    let end = *interval.end();
    if !start.is_finite()
        || !end.is_finite()
        || start >= end
        || start < *domain.start()
        || end > *domain.end()
    {
        Err(GeometryError::InvalidCurveTrimInterval)
    } else {
        Ok(())
    }
}

/// Which one-sided limit to evaluate at a curve knot, composite junction, or
/// surface knot line. Domain endpoints use the only available interior side.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ParameterSide {
    Left,
    #[default]
    Right,
}

pub(crate) fn check_interval(domain: &RangeInclusive<Real>) -> Result<(), GeometryError> {
    let start = *domain.start();
    let end = *domain.end();
    if !start.is_finite() || !end.is_finite() || end <= start || !(end - start).is_finite() {
        return Err(GeometryError::InvalidCurveParameterInterval);
    }
    Ok(())
}

pub(crate) fn checked_parameter(
    parameter: Real,
    domain: RangeInclusive<Real>,
) -> Result<(), GeometryError> {
    require_finite([parameter], "curve parameter")?;
    if domain.contains(&parameter) {
        Ok(())
    } else {
        Err(GeometryError::ParameterOutOfDomain {
            parameter,
            domain_start: *domain.start(),
            domain_end: *domain.end(),
        })
    }
}

pub(crate) fn map_parameter(
    value: Real,
    source: RangeInclusive<Real>,
    target: RangeInclusive<Real>,
) -> Result<Real, GeometryError> {
    checked_parameter(value, source.clone())?;
    if value == *source.start() {
        return Ok(*target.start());
    }
    if value == *source.end() {
        return Ok(*target.end());
    }
    let from_start = value - *source.start();
    let from_end = *source.end() - value;
    let numerator = *target.end() - *target.start();
    let denominator = *source.end() - *source.start();
    let result = if from_start <= from_end {
        *target.start() + scaled_ratio(from_start, numerator, denominator)?
    } else {
        *target.end() - scaled_ratio(from_end, numerator, denominator)?
    };
    require_finite([result], "curve parameter mapping")?;
    Ok(result.clamp(*target.start(), *target.end()))
}

pub(crate) fn scaled_ratio(
    value: Real,
    numerator: Real,
    denominator: Real,
) -> Result<Real, GeometryError> {
    if value == 0.0 {
        return Ok(0.0);
    }
    if numerator == 1.0 {
        let result = value / denominator;
        require_finite([result], "curve derivative")?;
        return Ok(result);
    }
    let ratio = numerator / denominator;
    let product = value * numerator;
    let quotient = value / denominator;
    let orders = [
        (ratio, value * ratio),
        (product, product / denominator),
        (quotient, quotient * numerator),
    ];
    // Prefer a normal intermediate so its subnormal rounding is not magnified
    // later. Try every ordering before rejecting a representable final value.
    let result = orders
        .iter()
        .find(|(intermediate, result)| {
            intermediate.is_normal() && result.is_finite() && *result != 0.0
        })
        .or_else(|| orders.iter().find(|(_, result)| result.is_finite()))
        .map(|(_, result)| *result)
        .ok_or(GeometryError::NonFinite {
            context: "curve derivative",
        })?;
    require_finite([result], "curve derivative")?;
    Ok(result)
}

pub(crate) fn shifted_parameter(
    parameter: Real,
    domain: &RangeInclusive<Real>,
) -> Result<Real, GeometryError> {
    let result = parameter + (domain.end() - domain.start());
    require_finite([result], "shifted curve parameter")?;
    Ok(result)
}

pub(crate) fn wrapped_parameter(
    parameter: Real,
    domain: &RangeInclusive<Real>,
) -> Result<Real, GeometryError> {
    require_finite([parameter], "curve seam parameter")?;
    check_interval(domain)?;
    if domain.contains(&parameter) {
        return Ok(parameter);
    }
    let width = domain.end() - domain.start();
    let difference = parameter - domain.start();
    let offset = if difference.is_finite() {
        difference.rem_euclid(width)
    } else {
        (parameter.rem_euclid(width) - domain.start().rem_euclid(width)).rem_euclid(width)
    };
    Ok((domain.start() + offset).clamp(*domain.start(), *domain.end()))
}
