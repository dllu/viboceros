use crate::{GeometryError, Real, require_finite, vector::product_three};

const MAX_ADAPTIVE_DEPTH: u32 = 24;
const MAX_ADAPTIVE_INTERVALS: usize = 65_536;

// Positive abscissae and weights for the embedded Gauss 7 / Kronrod 15 rule.
const KRONROD_ABSCISSAE: [Real; 8] = [
    0.991_455_371_120_812_6,
    0.949_107_912_342_758_5,
    0.864_864_423_359_769_1,
    0.741_531_185_599_394_5,
    0.586_087_235_467_691_1,
    0.405_845_151_377_397_2,
    0.207_784_955_007_898_48,
    0.0,
];
const KRONROD_WEIGHTS: [Real; 8] = [
    0.022_935_322_010_529_224,
    0.063_092_092_629_978_56,
    0.104_790_010_322_250_19,
    0.140_653_259_715_525_92,
    0.169_004_726_639_267_9,
    0.190_350_578_064_785_42,
    0.204_432_940_075_298_89,
    0.209_482_141_084_727_82,
];
const GAUSS_WEIGHTS: [Real; 4] = [
    0.129_484_966_168_869_7,
    0.279_705_391_489_276_64,
    0.381_830_050_505_118_9,
    0.417_959_183_673_469_4,
];

pub(crate) fn integrate_adaptive(
    start: Real,
    end: Real,
    absolute_tolerance: Real,
    relative_tolerance: Real,
    mut integrand: impl FnMut(Real) -> Result<Real, GeometryError>,
) -> Result<Real, GeometryError> {
    require_finite(
        [start, end, absolute_tolerance, relative_tolerance],
        "integration inputs",
    )?;
    if start >= end || absolute_tolerance <= 0.0 || relative_tolerance <= 0.0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let mut remaining_intervals = MAX_ADAPTIVE_INTERVALS;
    adaptive_interval(
        start,
        end,
        absolute_tolerance,
        relative_tolerance,
        0,
        &mut remaining_intervals,
        &mut integrand,
    )
}

fn adaptive_interval(
    start: Real,
    end: Real,
    absolute_tolerance: Real,
    relative_tolerance: Real,
    depth: u32,
    remaining_intervals: &mut usize,
    integrand: &mut impl FnMut(Real) -> Result<Real, GeometryError>,
) -> Result<Real, GeometryError> {
    let Some(remaining) = remaining_intervals.checked_sub(1) else {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    };
    *remaining_intervals = remaining;
    let estimate = gauss_kronrod_15(start, end, integrand)?;
    let target = absolute_tolerance.max(relative_tolerance * estimate.value.abs());
    if estimate.error <= target {
        return Ok(estimate.value);
    }
    if depth >= MAX_ADAPTIVE_DEPTH {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let midpoint = start * 0.5 + end * 0.5;
    if midpoint <= start || midpoint >= end {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let child_tolerance = absolute_tolerance * 0.5;
    if child_tolerance <= 0.0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let left = adaptive_interval(
        start,
        midpoint,
        child_tolerance,
        relative_tolerance,
        depth + 1,
        remaining_intervals,
        integrand,
    )?;
    let right = adaptive_interval(
        midpoint,
        end,
        child_tolerance,
        relative_tolerance,
        depth + 1,
        remaining_intervals,
        integrand,
    )?;
    let value = left + right;
    require_finite([value], "numerical integral")?;
    Ok(value)
}

struct IntegrationEstimate {
    value: Real,
    error: Real,
}

fn gauss_kronrod_15(
    start: Real,
    end: Real,
    integrand: &mut impl FnMut(Real) -> Result<Real, GeometryError>,
) -> Result<IntegrationEstimate, GeometryError> {
    let center = start * 0.5 + end * 0.5;
    let half_width = end * 0.5 - start * 0.5;
    require_finite([center, half_width], "integration interval")?;
    if half_width <= 0.0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }

    let center_value = checked_integrand(integrand(center)?)?;
    let mut value_scale = center_value.abs();
    let mut pairs = [[0.0; 2]; 7];
    for index in 0..7 {
        let offset = half_width * KRONROD_ABSCISSAE[index];
        let left = checked_integrand(integrand(center - offset)?)?;
        let right = checked_integrand(integrand(center + offset)?)?;
        pairs[index] = [left, right];
        value_scale = value_scale.max(left.abs()).max(right.abs());
    }
    if value_scale == 0.0 {
        return Ok(IntegrationEstimate {
            value: 0.0,
            error: 0.0,
        });
    }

    let center_scaled = center_value / value_scale;
    let mut gauss_sum = GAUSS_WEIGHTS[3] * center_scaled;
    let mut kronrod_sum = KRONROD_WEIGHTS[7] * center_scaled;
    let mut absolute_sum = KRONROD_WEIGHTS[7] * center_scaled.abs();
    let mut scaled_pairs = [[0.0; 2]; 7];
    for (index, [left, right]) in pairs.into_iter().enumerate() {
        let left = left / value_scale;
        let right = right / value_scale;
        scaled_pairs[index] = [left, right];
        let pair_sum = left + right;
        kronrod_sum += KRONROD_WEIGHTS[index] * pair_sum;
        absolute_sum += KRONROD_WEIGHTS[index] * (left.abs() + right.abs());
        if index % 2 == 1 {
            gauss_sum += GAUSS_WEIGHTS[(index - 1) / 2] * pair_sum;
        }
    }

    let mean = kronrod_sum * 0.5;
    let mut ascending_sum = KRONROD_WEIGHTS[7] * (center_scaled - mean).abs();
    for (index, [left, right]) in scaled_pairs.into_iter().enumerate() {
        ascending_sum += KRONROD_WEIGHTS[index] * ((left - mean).abs() + (right - mean).abs());
    }
    let value = kronrod_sum.signum()
        * product_three(
            kronrod_sum.abs(),
            value_scale,
            half_width,
            "numerical integral",
        )?;
    let absolute_integral = product_three(
        absolute_sum,
        value_scale,
        half_width,
        "absolute numerical integral",
    )?;
    let ascending_integral = product_three(
        ascending_sum,
        value_scale,
        half_width,
        "ascending numerical integral",
    )?;
    let mut error_unit = (kronrod_sum - gauss_sum).abs();
    if ascending_sum > 0.0 && error_unit > 0.0 {
        let scale = (200.0 * error_unit / ascending_sum).powf(1.5).min(1.0);
        error_unit = ascending_sum * scale;
    }
    let mut error = product_three(
        error_unit,
        value_scale,
        half_width,
        "numerical integration error",
    )?;
    error = error.max(50.0 * Real::EPSILON * absolute_integral);
    require_finite(
        [value, error, absolute_integral, ascending_integral],
        "numerical integration estimate",
    )?;
    Ok(IntegrationEstimate { value, error })
}

fn checked_integrand(value: Real) -> Result<Real, GeometryError> {
    require_finite([value], "integration function value")?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrates_polynomials_and_smooth_transcendentals() {
        let polynomial = integrate_adaptive(0.0, 1.0, 1.0e-13, 1.0e-13, |x| Ok(x.powi(6))).unwrap();
        assert!((polynomial - 1.0 / 7.0).abs() <= 1.0e-13);

        let sine = integrate_adaptive(0.0, std::f64::consts::PI, 1.0e-13, 1.0e-13, |x| Ok(x.sin()))
            .unwrap();
        assert!((sine - 2.0).abs() <= 1.0e-13);

        let narrow = integrate_adaptive(0.0, 1.0e-308, 1.0e-12, 1.0e-12, |_| Ok(1.0e308)).unwrap();
        assert!((narrow - 1.0).abs() <= 1.0e-12);
    }

    #[test]
    fn rejects_invalid_intervals_and_nonfinite_integrands() {
        assert!(integrate_adaptive(1.0, 0.0, 1.0e-9, 1.0e-12, |_| Ok(1.0)).is_err());
        assert!(integrate_adaptive(0.0, 1.0, 1.0e-9, 1.0e-12, |_| Ok(Real::NAN)).is_err());
    }
}
