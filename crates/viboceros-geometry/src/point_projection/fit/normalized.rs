//! Exactly centered matrix entries with a shared integer denominator.
use super::*;
use crate::binary_accumulator::decompose;
use faer::Mat;
use num_bigint::BigInt;

pub(super) fn matrix(
    points: &[Point3],
    center: &[Rational; 3],
    scale: &Rational,
) -> Result<Mat<f64>, GeometryError> {
    // Every input is an integer times this quantum. Compacting away common
    // trailing binary zeros keeps ordinary models out of full-range bigints.
    let base = points
        .iter()
        .flat_map(|p| p.to_array())
        .filter(|v| *v != 0.)
        .map(|v| decompose(v).1)
        .min()
        .unwrap();
    let quantum = if base >= 1074 {
        Rational::from_integer(BigInt::from(1) << (base - 1074))
    } else {
        Rational::new(1.into(), BigInt::from(1) << (1074 - base))
    };
    let count = BigInt::from(points.len());
    let rational_count = Rational::from_integer(count.clone());
    let totals: [BigInt; 3] = std::array::from_fn(|i| {
        let total = &center[i] * &rational_count / &quantum;
        debug_assert!(total.is_integer());
        total.to_integer()
    });
    let span = scale / &quantum;
    debug_assert!(span.is_integer() && span > Rational::zero());
    let denominator = span.to_integer() * &count;
    let mut matrix = Mat::zeros(points.len(), 3);
    for (row, point) in points.iter().enumerate() {
        for (column, value) in point.to_array().into_iter().enumerate() {
            let (mantissa, shift) = decompose(value);
            let integer = if mantissa == 0 {
                BigInt::from(0)
            } else {
                let value_integer = BigInt::from(mantissa) << (shift - base);
                if value.is_sign_negative() {
                    -value_integer
                } else {
                    value_integer
                }
            };
            let numerator = integer * &count - &totals[column];
            let nonzero = !numerator.is_zero();
            // Only conversion is applied to this unreduced ratio. Its positive
            // denominator is shared and proven nonzero; no GCD is needed for
            // correctly rounded num-rational binary64 conversion.
            let value = scalar(&Rational::new_raw(numerator, denominator.clone()))?;
            if (nonzero && value == 0.) || value.is_subnormal() {
                return Err(GeometryError::PlaneFitNumericalRange);
            }
            matrix[(row, column)] = value;
        }
    }
    Ok(matrix)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_integer_entries_match_independent_rational_centering_bit_for_bit() {
        let mut state = 137_u64;
        for count in [4, 5, 17, 100] {
            for exponent in [-1000, -53, 0, 53, 1000] {
                let points = (0..count)
                    .map(|_| {
                        Point3::try_from(std::array::from_fn(|_| {
                            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                            (state as i64 as f64 / 2_f64.powi(63)) * 2_f64.powi(exponent)
                        }))
                        .unwrap()
                    })
                    .collect::<Vec<_>>();
                let center: [Rational; 3] = std::array::from_fn(|i| {
                    points
                        .iter()
                        .map(|p| rational(p.to_array()[i]))
                        .sum::<Rational>()
                        / Rational::from_integer(count.into())
                });
                let scale = (0..3)
                    .map(|i| {
                        let a = points
                            .iter()
                            .map(|p| p.to_array()[i])
                            .min_by(f64::total_cmp)
                            .unwrap();
                        let b = points
                            .iter()
                            .map(|p| p.to_array()[i])
                            .max_by(f64::total_cmp)
                            .unwrap();
                        rational(b) - rational(a)
                    })
                    .max()
                    .unwrap();
                let actual = matrix(&points, &center, &scale).unwrap();
                for (i, p) in points.iter().enumerate() {
                    for j in 0..3 {
                        let expected =
                            scalar(&((rational(p.to_array()[j]) - &center[j]) / &scale)).unwrap();
                        assert_eq!(actual[(i, j)].to_bits(), expected.to_bits());
                    }
                }
            }
        }
    }
}
