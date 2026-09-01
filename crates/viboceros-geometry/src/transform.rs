use nalgebra::Matrix3;

use crate::{GeometryError, Point3, Real, Vector3, require_finite};

/// A finite affine map from three-dimensional model space to itself.
///
/// The linear part is stored with nalgebra, while application uses the
/// kernel's scaled dot product to avoid spurious intermediate overflow when
/// large products cancel to a representable coordinate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AffineTransform3 {
    linear: Matrix3<Real>,
    translation: Vector3,
}

impl AffineTransform3 {
    pub fn identity() -> Self {
        Self {
            linear: Matrix3::identity(),
            translation: Vector3::try_new(0.0, 0.0, 0.0).expect("the zero translation is finite"),
        }
    }

    pub fn from_translation(translation: Vector3) -> Self {
        Self {
            linear: Matrix3::identity(),
            translation,
        }
    }

    pub fn try_new(
        linear_rows: [[Real; 3]; 3],
        translation: Vector3,
    ) -> Result<Self, GeometryError> {
        require_finite(linear_rows.iter().flatten().copied(), "affine transform")?;
        Ok(Self {
            linear: Matrix3::from_row_slice(&linear_rows.concat()),
            translation,
        })
    }

    pub fn linear_rows(self) -> [[Real; 3]; 3] {
        std::array::from_fn(|row| std::array::from_fn(|column| self.linear[(row, column)]))
    }

    pub const fn translation(self) -> Vector3 {
        self.translation
    }

    pub fn transform_point(self, point: Point3) -> Result<Point3, GeometryError> {
        let linear = self.linear_coordinates(Vector3::try_from(point.to_array())?)?;
        let transformed = [
            linear[0] + self.translation.x(),
            linear[1] + self.translation.y(),
            linear[2] + self.translation.z(),
        ];
        require_finite(transformed, "transformed point")?;
        Point3::try_from(transformed)
    }

    pub fn transform_vector(self, vector: Vector3) -> Result<Vector3, GeometryError> {
        Vector3::try_from(self.linear_coordinates(vector)?)
    }

    fn linear_coordinates(self, vector: Vector3) -> Result<[Real; 3], GeometryError> {
        let mut coordinates = [0.0; 3];
        for (row, coordinate) in coordinates.iter_mut().enumerate() {
            let coefficients = Vector3::try_new(
                self.linear[(row, 0)],
                self.linear[(row, 1)],
                self.linear[(row, 2)],
            )?;
            *coordinate = coefficients.dot(vector)?;
        }
        Ok(coordinates)
    }
}

impl Default for AffineTransform3 {
    fn default() -> Self {
        Self::identity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tolerance;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn identity_and_translation_distinguish_points_from_vectors() {
        let original = point(1.0, 2.0, 3.0);
        assert_eq!(
            AffineTransform3::identity()
                .transform_point(original)
                .unwrap(),
            original
        );

        let offset = Vector3::try_new(4.0, -5.0, 6.0).unwrap();
        let transform = AffineTransform3::from_translation(offset);
        assert_eq!(
            transform.transform_point(original).unwrap(),
            point(5.0, -3.0, 9.0)
        );
        assert_eq!(transform.transform_vector(offset).unwrap(), offset);
    }

    #[test]
    fn applies_a_general_finite_linear_part() {
        let transform = AffineTransform3::try_new(
            [[0.0, -2.0, 0.0], [3.0, 0.0, 0.0], [0.0, 0.0, 4.0]],
            Vector3::try_new(10.0, 20.0, 30.0).unwrap(),
        )
        .unwrap();
        assert_eq!(
            transform.transform_point(point(1.0, 2.0, 3.0)).unwrap(),
            point(6.0, 23.0, 42.0)
        );
        assert_eq!(
            transform
                .transform_vector(Vector3::try_new(1.0, 2.0, 3.0).unwrap())
                .unwrap(),
            Vector3::try_new(-4.0, 3.0, 12.0).unwrap()
        );
    }

    #[test]
    fn scaled_dot_product_preserves_large_cancellation() {
        let transform = AffineTransform3::try_new(
            [[1.0, 1.0, -1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        let transformed = transform
            .transform_point(point(Real::MAX, Real::MAX, Real::MAX))
            .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(transformed.x(), Real::MAX));
        assert_eq!(transformed.y(), Real::MAX);
        assert_eq!(transformed.z(), Real::MAX);
    }

    #[test]
    fn rejects_non_finite_coefficients_and_outputs() {
        assert!(
            AffineTransform3::try_new(
                [[Real::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
            )
            .is_err()
        );
        let translation =
            AffineTransform3::from_translation(Vector3::try_new(Real::MAX, 0.0, 0.0).unwrap());
        assert!(
            translation
                .transform_point(point(Real::MAX, 0.0, 0.0))
                .is_err()
        );
    }
}
