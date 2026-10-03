//! Cofactor normal directions, including singular maps and extreme coefficients.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use num_traits::{Signed, Zero};

/// Maps oriented surface/mesh normals by the linear cofactor matrix. For an
/// invertible map this is det(A) A^-T, up to a positive common scale. B-rep
/// orientation policy can negate the result for an orientation-reversing map.
/// Rank-two maps retain surviving face normals; collapsed faces return an error.
#[derive(Clone, Copy, Debug)]
pub struct AffineNormalTransform3 {
    original: [[Real; 3]; 3],
    coefficients: [[Real; 3]; 3],
    exact: bool,
}

fn cofactors(rows: [[Real; 3]; 3]) -> [[Rational; 3]; 3] {
    let rows = rows.map(|row| row.map(rational));
    std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            &rows[(i + 1) % 3][(j + 1) % 3] * &rows[(i + 2) % 3][(j + 2) % 3]
                - &rows[(i + 1) % 3][(j + 2) % 3] * &rows[(i + 2) % 3][(j + 1) % 3]
        })
    })
}

impl AffineNormalTransform3 {
    /// Prepare once per affine instance, without dividing by the determinant.
    pub fn new(transform: AffineTransform3) -> Self {
        let original = transform.linear_rows();
        let exact_coefficients = cofactors(original);
        let maximum = exact_coefficients
            .iter()
            .flatten()
            .map(Signed::abs)
            .max()
            .unwrap();
        let mut exact = false;
        let coefficients = if maximum.is_zero() {
            [[0.; 3]; 3]
        } else {
            std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    let value = scalar(&(&exact_coefficients[i][j] / &maximum))
                        .expect("bounded normal coefficient");
                    exact |= value == 0. && !exact_coefficients[i][j].is_zero();
                    value
                })
            })
        };
        Self {
            original,
            coefficients,
            exact,
        }
    }

    pub fn transform_normal(self, normal: Vector3) -> Result<UnitVector3, GeometryError> {
        let original_normal = normal;
        let normal = normal.normalized_nonzero()?.as_vector();
        if !self.exact {
            let coordinates = self
                .coefficients
                .map(|row| normal.dot(Vector3::try_from(row).unwrap()).unwrap());
            let bound = self
                .coefficients
                .iter()
                .map(|row| {
                    row.iter()
                        .zip(normal.to_array())
                        .map(|(a, b)| (a * b).abs())
                        .sum::<Real>()
                })
                .fold(0., Real::max);
            let mapped = Vector3::try_from(coordinates)?;
            // Near-null directions amplify coefficient rounding. Evaluate
            // those directions from the original binary64 coefficients.
            if mapped.length()? > bound * 1e-4 {
                return mapped.normalized_nonzero();
            }
        }
        let coefficients = cofactors(self.original);
        let normal = original_normal.to_array().map(rational);
        let coordinates = coefficients.map(|row| {
            row.iter()
                .zip(&normal)
                .map(|(a, b)| a * b)
                .sum::<Rational>()
        });
        let maximum = coordinates.iter().map(Signed::abs).max().unwrap();
        if maximum.is_zero() {
            return Err(GeometryError::Degenerate {
                context: "transformed surface normal",
            });
        }
        Vector3::try_from(
            coordinates.map(|value| scalar(&(value / &maximum)).expect("bounded normal component")),
        )?
        .normalized_nonzero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vector(values: [Real; 3]) -> Vector3 {
        Vector3::try_from(values).unwrap()
    }
    fn map(rows: [[Real; 3]; 3]) -> AffineNormalTransform3 {
        AffineNormalTransform3::new(AffineTransform3::try_new(rows, vector([0.; 3])).unwrap())
    }
    fn close(a: Vector3, b: Vector3) {
        for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
            assert!((a - b).abs() < 1e-12, "{a} != {b}");
        }
    }
    #[test]
    fn cofactor_normals_agree_with_transformed_tangent_cross_products() {
        let transforms = [
            [[2., 0., 0.], [0., 3., 0.], [0., 0., 0.5]],
            [[1., 3., 0.], [0., 1., 0.], [0., 0., 1.]],
            [[-2., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
        ];
        for rows in transforms {
            let transform = AffineTransform3::try_new(rows, vector([0.; 3])).unwrap();
            for (u, v) in [
                ([1., 2., 0.], [0., 1., 3.]),
                ([1., 0., 0.], [0., 1., 1.]),
                ([0., 1., 0.], [0., 0., 1.]),
            ] {
                let (u, v) = (vector(u), vector(v));
                let normal = u.cross(v).unwrap();
                let expected = transform
                    .transform_vector(u)
                    .unwrap()
                    .cross(transform.transform_vector(v).unwrap())
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap();
                close(
                    map(rows).transform_normal(normal).unwrap().as_vector(),
                    expected.as_vector(),
                );
            }
        }
    }
    #[test]
    fn normal_mapping_handles_mixed_extremes_tiny_scales_and_exact_cancellation() {
        for rows in [
            [[1e308, 0., 0.], [0., 1e-308, 0.], [0., 0., 1.]],
            [[1e-308, 0., 0.], [0., 1e-308, 0.], [0., 0., 1e-308]],
            [[1e308, 0., 0.], [0., 1e308, 0.], [0., 0., 1e308]],
        ] {
            for axis in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]] {
                close(
                    map(rows)
                        .transform_normal(vector(axis))
                        .unwrap()
                        .as_vector(),
                    vector(axis),
                );
            }
        }
        let n = 2_f64.powi(52);
        let rows = [[n, n - 1., 0.], [n + 1., n, 0.], [0., 0., 1.]];
        // The exact 2x2 minor is one; ordinary rounded products cancel to zero.
        close(
            map(rows)
                .transform_normal(vector([0., 0., 1.]))
                .unwrap()
                .as_vector(),
            vector([0., 0., 1.]),
        );
    }
    #[test]
    fn singular_maps_keep_surviving_faces_and_reject_collapsed_normals() {
        let normals = map([[1., 0., 0.], [0., 1., 0.], [0., 0., 0.]]);
        close(
            normals
                .transform_normal(vector([0., 0., 1.]))
                .unwrap()
                .as_vector(),
            vector([0., 0., 1.]),
        );
        assert!(normals.transform_normal(vector([1., 0., 0.])).is_err());
        assert!(
            map([[0.; 3]; 3])
                .transform_normal(vector([0., 0., 1.]))
                .is_err()
        );
    }
    #[test]
    fn near_null_normals_use_original_components_before_unitizing() {
        let normals = map([[1., 1., 0.], [0., 0., 0.], [0., 0., 1.]]);
        for n in [
            [1., 1_f64.next_up(), 0.],
            [1e308, 1e308_f64.next_up(), 0.],
            [1e-308, 1e-308_f64.next_up(), 0.],
        ] {
            close(
                normals.transform_normal(vector(n)).unwrap().as_vector(),
                vector([0., 1., 0.]),
            );
        }
        let normals = map([[1., 0., 0.], [0., 0., 0.], [0., 0., 1.]]);
        close(
            normals
                .transform_normal(vector([1e308, 1e-308, 0.]))
                .unwrap()
                .as_vector(),
            vector([0., 1., 0.]),
        );
    }
}
