use crate::{FiniteSum, GeometryError, Real, require_finite, vector::product_three};
use std::{cmp::Ordering, collections::BinaryHeap};

pub(crate) mod cash_karp;

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
    let estimate = gauss_kronrod_15(start, end, &mut integrand)?;
    if estimate.error <= absolute_tolerance.max(relative_tolerance * estimate.value.abs()) {
        return Ok(estimate.value);
    }
    let mut values = FiniteSum::default();
    let mut errors = FiniteSum::default();
    values.add(estimate.value)?;
    errors.add(estimate.error)?;
    let mut intervals = BinaryHeap::from([AdaptiveInterval {
        start,
        end,
        estimate,
    }]);
    let mut evaluated = 1;
    loop {
        let value = values.total()?;
        let error = errors.total()?;
        let target = absolute_tolerance.max(relative_tolerance * value.abs());
        if error <= target {
            return Ok(value);
        }
        if evaluated + 2 > MAX_ADAPTIVE_INTERVALS {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        let parent = intervals
            .pop()
            .ok_or(GeometryError::NumericalIntegrationDidNotConverge)?;
        let midpoint = parent.start.midpoint(parent.end);
        if midpoint <= parent.start || midpoint >= parent.end {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        let left = gauss_kronrod_15(parent.start, midpoint, &mut integrand)?;
        let right = gauss_kronrod_15(midpoint, parent.end, &mut integrand)?;
        evaluated += 2;
        // A stable interval at its roundoff floor cannot improve by splitting.
        // Keep its estimate in the global sums, but allow other intervals to
        // reduce their errors. An empty refinement heap rejects unattainable
        // precision promptly. Larger child errors reveal new uncertainty and
        // must still be admitted, even when the parent was at its floor.
        let mut change = FiniteSum::default();
        change.add(left.value)?;
        change.add(right.value)?;
        change.add(-parent.estimate.value)?;
        let child_error = left.error + right.error;
        if parent.estimate.error <= 64. * Real::EPSILON * parent.estimate.absolute
            && child_error >= 0.99 * parent.estimate.error
            && child_error <= 1.01 * parent.estimate.error
            && change.total()?.abs() <= 64. * Real::EPSILON * parent.estimate.absolute
        {
            continue;
        }
        values.add(-parent.estimate.value)?;
        values.add(left.value)?;
        values.add(right.value)?;
        errors.add(-parent.estimate.error)?;
        errors.add(left.error)?;
        errors.add(right.error)?;
        intervals.push(AdaptiveInterval {
            start: parent.start,
            end: midpoint,
            estimate: left,
        });
        intervals.push(AdaptiveInterval {
            start: midpoint,
            end: parent.end,
            estimate: right,
        });
    }
}

struct AdaptiveInterval {
    start: Real,
    end: Real,
    estimate: IntegrationEstimate,
}

impl Ord for AdaptiveInterval {
    fn cmp(&self, other: &Self) -> Ordering {
        self.estimate
            .error
            .total_cmp(&other.estimate.error)
            .then_with(|| other.start.total_cmp(&self.start))
            .then_with(|| other.end.total_cmp(&self.end))
    }
}
impl PartialOrd for AdaptiveInterval {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for AdaptiveInterval {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for AdaptiveInterval {}

struct IntegrationEstimate {
    value: Real,
    error: Real,
    absolute: Real,
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
            absolute: 0.0,
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
    Ok(IntegrationEstimate {
        value,
        error,
        absolute: absolute_integral,
    })
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

    #[test]
    fn localized_non_dyadic_cusp_uses_a_global_error_budget() {
        let mut evaluations = 0;
        let value = integrate_adaptive(0., 1., 1e-12, 1e-12, |x| {
            evaluations += 1;
            Ok((x - 0.4).abs())
        })
        .unwrap();
        assert!((value - 0.26).abs() <= 1e-12);
        assert!(evaluations < 1000, "{evaluations}");
    }

    #[test]
    fn signed_cancellation_keeps_the_global_absolute_tolerance() {
        let value =
            integrate_adaptive(0., 1., 1e-12, 1e-4, |x| Ok((x - 0.4).abs() - 0.26)).unwrap();
        assert!(value.abs() <= 1e-12, "{value}");
    }

    #[test]
    fn unattainable_roundoff_precision_fails_without_exhausting_the_work_budget() {
        let mut evaluations = 0;
        let result = integrate_adaptive(0., 1., 1e-30, 1e-30, |_| {
            evaluations += 1;
            Ok(1.)
        });
        assert_eq!(
            result,
            Err(GeometryError::NumericalIntegrationDidNotConverge)
        );
        assert!(evaluations <= 45, "{evaluations}");
    }

    #[test]
    fn a_roundoff_limited_interval_does_not_block_refinement_elsewhere() {
        let value = integrate_adaptive(0., 1., 1e-4, 1e-16, |x| {
            Ok(if x < 0.5 {
                1e10
            } else {
                (20000. * (x - 0.5)).sin()
            })
        })
        .unwrap();
        let expected = 5e9 + (1. - 10000_f64.cos()) / 20000.;
        assert!((value - expected).abs() <= 1e-4, "{value} != {expected}");
    }
}
