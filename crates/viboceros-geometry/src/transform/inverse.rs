//! Rounded affine inverses from exact binary64 coefficients.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use num_traits::{Signed, Zero};

impl AffineTransform3 {
    /// Invert the exact input coefficients, with one final rounding per output
    /// coefficient. Reject exact singularity, overflow and an inverse whose
    /// rounded linear part loses or reverses the expected orientation.
    /// Ill-conditioned inputs do not carry a round-trip accuracy guarantee.
    pub fn try_inverse(self) -> Result<Self, GeometryError> {
        let rows = self.linear_rows().map(|row| row.map(rational));
        let cofactors: [[Rational; 3]; 3] = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                &rows[(i + 1) % 3][(j + 1) % 3] * &rows[(i + 2) % 3][(j + 2) % 3]
                    - &rows[(i + 1) % 3][(j + 2) % 3] * &rows[(i + 2) % 3][(j + 1) % 3]
            })
        });
        let determinant = (0..3)
            .map(|j| &rows[0][j] * &cofactors[0][j])
            .sum::<Rational>();
        if determinant.is_zero() {
            return Err(GeometryError::Degenerate {
                context: "affine inverse",
            });
        }
        let inverse: [[Rational; 3]; 3] =
            std::array::from_fn(|i| std::array::from_fn(|j| &cofactors[j][i] / &determinant));
        let mut linear = [[0.; 3]; 3];
        let mut translation = [0.; 3];
        let original_translation = self.translation().to_array().map(rational);
        for i in 0..3 {
            for j in 0..3 {
                linear[i][j] = scalar(&inverse[i][j])?;
            }
            translation[i] = scalar(
                &-(0..3)
                    .map(|j| &inverse[i][j] * &original_translation[j])
                    .sum::<Rational>(),
            )?;
        }
        let result = Self::try_new(linear, Vector3::try_from(translation)?)?;
        if result.orientation_reversing()? != determinant.is_negative() {
            return Err(GeometryError::Degenerate {
                context: "rounded affine inverse",
            });
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn affine(rows: [[f64; 3]; 3], t: [f64; 3]) -> AffineTransform3 {
        AffineTransform3::try_new(rows, Vector3::try_from(t).unwrap()).unwrap()
    }
    #[test]
    fn reflected_nonuniform_rotation_retains_placement_in_both_orders() {
        let transform = affine(
            [[0., -2., 0.], [3., 0., 0.], [0., 0., -4.]],
            [10., 20., 30.],
        );
        let inverse = transform.try_inverse().unwrap();
        assert_eq!(
            inverse.linear_rows(),
            [[0., 1. / 3., 0.], [-0.5, 0., 0.], [0., 0., -0.25]]
        );
        for xyz in [[0., 0., 0.], [1., 2., 3.], [-4., 7., -2.]] {
            let point = Point3::try_from(xyz).unwrap();
            for (first, second) in [(transform, inverse), (inverse, transform)] {
                assert!(
                    second
                        .transform_point(first.transform_point(point).unwrap())
                        .unwrap()
                        .distance_to(point)
                        .unwrap()
                        < 1e-12
                );
            }
        }
    }
    #[test]
    fn cancellation_that_rounds_a_determinant_to_zero_remains_invertible() {
        let n = 2_f64.powi(27);
        let transform = affine([[n, n - 1., 0.], [n + 1., n, 0.], [0., 0., 1.]], [0.; 3]);
        assert_eq!(n * n - (n - 1.) * (n + 1.), 0.);
        let inverse = transform.try_inverse().unwrap();
        assert_eq!(
            inverse.linear_rows(),
            [[n, 1. - n, 0.], [-n - 1., n, 0.], [0., 0., 1.]]
        );
        let point = Point3::try_new(1., 1., 5.).unwrap();
        assert_eq!(
            inverse
                .transform_point(transform.transform_point(point).unwrap())
                .unwrap(),
            point
        );
    }
    #[test]
    fn extreme_scales_and_overflowing_translation_terms_keep_finite_inverses() {
        let huge = 2_f64.powi(1023);
        let tiny = 2_f64.powi(-1022);
        let inverse = affine([[huge, 0., 0.], [0., tiny, 0.], [0., 0., 1.]], [0.; 3])
            .try_inverse()
            .unwrap();
        assert_eq!(
            inverse.linear_rows(),
            [[1. / huge, 0., 0.], [0., 1. / tiny, 0.], [0., 0., 1.]]
        );
        let inverse = affine(
            [[0.5, 1., 0.], [0., 1., 0.], [0., 0., 1.]],
            [f64::MAX, f64::MAX, 0.],
        )
        .try_inverse()
        .unwrap();
        assert_eq!(inverse.translation().to_array(), [0., -f64::MAX, 0.]);
    }
    #[test]
    fn singular_or_unrepresentable_inverses_are_errors() {
        for transform in [
            affine([[0.; 3]; 3], [0.; 3]),
            affine([[1., 2., 3.], [2., 4., 6.], [0., 0., 1.]], [0.; 3]),
            affine(
                [[f64::from_bits(1), 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                [0.; 3],
            ),
            affine(
                [[0.5, 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                [f64::MAX, 0., 0.],
            ),
        ] {
            assert!(transform.try_inverse().is_err());
        }
    }
}
