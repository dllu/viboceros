use nalgebra::Matrix3;

use crate::{Frame3, GeometryError, Point3, Real, UnitVector3, Vector3, require_finite};

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

    pub fn try_uniform_scale(fixed_point: Point3, factor: Real) -> Result<Self, GeometryError> {
        require_finite([factor], "scale factor")?;
        Self::try_with_fixed_point(
            [[factor, 0.0, 0.0], [0.0, factor, 0.0], [0.0, 0.0, factor]],
            fixed_point,
        )
    }

    /// Scales independently along the world coordinate axes while retaining
    /// a fixed point. Zero factors are valid projections onto a plane or axis.
    pub fn try_nonuniform_scale(
        fixed_point: Point3,
        factors: [Real; 3],
    ) -> Result<Self, GeometryError> {
        require_finite(factors, "non-uniform scale factor")?;
        Self::try_with_fixed_point(
            [
                [factors[0], 0.0, 0.0],
                [0.0, factors[1], 0.0],
                [0.0, 0.0, factors[2]],
            ],
            fixed_point,
        )
    }

    /// Scales only the component parallel to a unit direction while retaining
    /// the perpendicular components and a fixed point.
    pub fn try_directional_scale(
        fixed_point: Point3,
        direction: UnitVector3,
        factor: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([factor], "directional scale factor")?;
        let direction = direction.as_vector().to_array();
        let delta = factor - 1.0;
        let linear = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                let identity = if row == column { 1.0 } else { 0.0 };
                delta.mul_add(direction[row] * direction[column], identity)
            })
        });
        Self::try_with_fixed_point(linear, fixed_point)
    }

    /// Shears along `shear_direction` in proportion to displacement along
    /// `reference_direction`, retaining `fixed_point`. The two unit directions
    /// must be perpendicular so the map preserves volume and remains a pure
    /// shear rather than including an axial scale.
    pub fn try_shear(
        fixed_point: Point3,
        reference_direction: UnitVector3,
        shear_direction: UnitVector3,
        factor: Real,
        tolerance: crate::Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([factor], "shear factor")?;
        let reference = reference_direction.as_vector();
        let shear = shear_direction.as_vector();
        if reference.dot(shear)?.abs() > tolerance.angular() {
            return Err(GeometryError::Degenerate {
                context: "shear directions",
            });
        }
        let reference = reference.to_array();
        let shear = shear.to_array();
        let linear = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                let identity = if row == column { 1.0 } else { 0.0 };
                factor.mul_add(shear[row] * reference[column], identity)
            })
        });
        Self::try_with_fixed_point(linear, fixed_point)
    }

    pub fn try_rotation(
        fixed_point: Point3,
        axis: UnitVector3,
        angle_radians: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([angle_radians], "rotation angle")?;
        let (sine, cosine) = angle_radians.sin_cos();
        let one_minus_cosine = 1.0 - cosine;
        let [x, y, z] = axis.as_vector().to_array();
        let linear = [
            [
                x * x * one_minus_cosine + cosine,
                x * y * one_minus_cosine - z * sine,
                x * z * one_minus_cosine + y * sine,
            ],
            [
                y * x * one_minus_cosine + z * sine,
                y * y * one_minus_cosine + cosine,
                y * z * one_minus_cosine - x * sine,
            ],
            [
                z * x * one_minus_cosine - y * sine,
                z * y * one_minus_cosine + x * sine,
                z * z * one_minus_cosine + cosine,
            ],
        ];
        Self::try_with_fixed_point(linear, fixed_point)
    }

    /// Returns the shortest proper rotation that maps one unit direction to
    /// another. Antiparallel inputs use a deterministic perpendicular axis.
    pub fn try_rotation_between(
        from: UnitVector3,
        to: UnitVector3,
        tolerance: crate::Tolerance,
    ) -> Result<Self, GeometryError> {
        let from_vector = from.as_vector();
        let to_vector = to.as_vector();
        let cosine = from_vector.dot(to_vector)?.clamp(-1.0, 1.0);
        let cross = from_vector.cross(to_vector)?;
        let sine = cross.length()?.clamp(0.0, 1.0);
        if sine > tolerance.angular() {
            let axis = cross.normalized_nonzero()?;
            let origin = Point3::try_new(0.0, 0.0, 0.0)?;
            return Self::try_rotation(origin, axis, sine.atan2(cosine));
        }
        if cosine >= 0.0 {
            return Ok(Self::identity());
        }
        let linear = {
            let [fx, fy, fz] = from_vector.to_array().map(Real::abs);
            let reference = if fx <= fy && fx <= fz {
                Vector3::try_new(1.0, 0.0, 0.0)?
            } else if fy <= fz {
                Vector3::try_new(0.0, 1.0, 0.0)?
            } else {
                Vector3::try_new(0.0, 0.0, 1.0)?
            };
            let axis = from_vector.cross(reference)?.normalized_nonzero()?;
            let [x, y, z] = axis.as_vector().to_array();
            [
                [2.0 * x * x - 1.0, 2.0 * x * y, 2.0 * x * z],
                [2.0 * y * x, 2.0 * y * y - 1.0, 2.0 * y * z],
                [2.0 * z * x, 2.0 * z * y, 2.0 * z * z - 1.0],
            ]
        };
        Self::try_new(
            linear,
            Vector3::try_new(0.0, 0.0, 0.0).expect("the zero translation is finite"),
        )
    }

    /// Maps one directed origin to another with independent scale along and
    /// perpendicular to the source direction. The rotational component is
    /// the shortest proper rotation between the directions.
    #[allow(clippy::too_many_arguments)]
    pub fn try_direction_mapping(
        source_origin: Point3,
        source_direction: UnitVector3,
        target_origin: Point3,
        target_direction: UnitVector3,
        axial_scale: Real,
        perpendicular_scale: Real,
        tolerance: crate::Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite(
            [axial_scale, perpendicular_scale],
            "direction mapping scale",
        )?;
        let rotation = Self::try_rotation_between(source_direction, target_direction, tolerance)?;
        let direction = source_direction.as_vector().to_array();
        let directional_scale = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                let projection = direction[row] * direction[column];
                let identity = if row == column { 1.0 } else { 0.0 };
                axial_scale.mul_add(projection, perpendicular_scale * (identity - projection))
            })
        });
        let linear = multiply_linear(rotation.linear_rows(), directional_scale)?;
        Self::try_mapping_origins(linear, source_origin, target_origin)
    }

    /// Maps one right-handed orthonormal frame to another, applying the
    /// requested scale factors along the source frame axes first.
    pub fn try_frame_mapping(
        source: Frame3,
        target: Frame3,
        scale_factors: [Real; 3],
    ) -> Result<Self, GeometryError> {
        require_finite(scale_factors, "frame mapping scale")?;
        let source_axes = source.axes();
        let target_axes = target.axes();
        let mut linear = [[0.0; 3]; 3];
        for (row, linear_row) in linear.iter_mut().enumerate() {
            let weighted_target = Vector3::try_new(
                target_axes[0].as_vector().to_array()[row] * scale_factors[0],
                target_axes[1].as_vector().to_array()[row] * scale_factors[1],
                target_axes[2].as_vector().to_array()[row] * scale_factors[2],
            )?;
            for (column, coefficient) in linear_row.iter_mut().enumerate() {
                *coefficient = weighted_target.dot(Vector3::try_new(
                    source_axes[0].as_vector().to_array()[column],
                    source_axes[1].as_vector().to_array()[column],
                    source_axes[2].as_vector().to_array()[column],
                )?)?;
            }
        }
        Self::try_mapping_origins(linear, source.origin(), target.origin())
    }

    pub fn try_reflection(
        point_on_plane: Point3,
        plane_normal: UnitVector3,
    ) -> Result<Self, GeometryError> {
        let [x, y, z] = plane_normal.as_vector().to_array();
        let linear = [
            [1.0 - 2.0 * x * x, -2.0 * x * y, -2.0 * x * z],
            [-2.0 * y * x, 1.0 - 2.0 * y * y, -2.0 * y * z],
            [-2.0 * z * x, -2.0 * z * y, 1.0 - 2.0 * z * z],
        ];
        Self::try_with_fixed_point(linear, point_on_plane)
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

    fn try_with_fixed_point(
        linear_rows: [[Real; 3]; 3],
        fixed_point: Point3,
    ) -> Result<Self, GeometryError> {
        let zero = Vector3::try_new(0.0, 0.0, 0.0)?;
        let linear_transform = Self::try_new(linear_rows, zero)?;
        let mapped_fixed_point = linear_transform.transform_point(fixed_point)?;
        let translation = mapped_fixed_point.vector_to(fixed_point)?;
        Self::try_new(linear_rows, translation)
    }

    fn try_mapping_origins(
        linear_rows: [[Real; 3]; 3],
        source_origin: Point3,
        target_origin: Point3,
    ) -> Result<Self, GeometryError> {
        let zero = Vector3::try_new(0.0, 0.0, 0.0)?;
        let linear_transform = Self::try_new(linear_rows, zero)?;
        let mapped_source = linear_transform.transform_point(source_origin)?;
        Self::try_new(linear_rows, mapped_source.vector_to(target_origin)?)
    }
}

fn multiply_linear(
    left: [[Real; 3]; 3],
    right: [[Real; 3]; 3],
) -> Result<[[Real; 3]; 3], GeometryError> {
    let mut product = [[0.0; 3]; 3];
    for (row, product_row) in product.iter_mut().enumerate() {
        let left_row = Vector3::try_from(left[row])?;
        for (column, coefficient) in product_row.iter_mut().enumerate() {
            *coefficient = left_row.dot(Vector3::try_new(
                right[0][column],
                right[1][column],
                right[2][column],
            )?)?;
        }
    }
    Ok(product)
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

    #[test]
    fn centered_uniform_scale_keeps_its_fixed_point() {
        let center = point(1.0, 2.0, 3.0);
        let transform = AffineTransform3::try_uniform_scale(center, 2.0).unwrap();
        assert_eq!(transform.transform_point(center).unwrap(), center);
        assert_eq!(
            transform.transform_point(point(2.0, 4.0, 6.0)).unwrap(),
            point(3.0, 6.0, 9.0)
        );
        assert!(AffineTransform3::try_uniform_scale(center, Real::NAN).is_err());
    }

    #[test]
    fn nonuniform_and_directional_scales_retain_their_fixed_point() {
        let center = point(1.0, 2.0, 3.0);
        let nonuniform = AffineTransform3::try_nonuniform_scale(center, [-2.0, 0.5, 0.0]).unwrap();
        assert_eq!(nonuniform.transform_point(center).unwrap(), center);
        assert_eq!(
            nonuniform.transform_point(point(2.0, 4.0, 6.0)).unwrap(),
            point(-1.0, 3.0, 3.0)
        );

        let direction = UnitVector3::try_new(1.0, 1.0, 0.0, Tolerance::DEFAULT).unwrap();
        let directional = AffineTransform3::try_directional_scale(center, direction, 3.0).unwrap();
        assert_eq!(directional.transform_point(center).unwrap(), center);
        assert!(
            directional
                .transform_point(point(2.0, 3.0, 4.0))
                .unwrap()
                .is_near(point(4.0, 5.0, 4.0), Tolerance::DEFAULT)
        );
        assert!(
            directional
                .transform_point(point(2.0, 1.0, 4.0))
                .unwrap()
                .is_near(point(2.0, 1.0, 4.0), Tolerance::DEFAULT)
        );

        let flattened = AffineTransform3::try_directional_scale(center, direction, 0.0).unwrap();
        assert!(
            flattened
                .transform_point(point(2.0, 3.0, 4.0))
                .unwrap()
                .is_near(point(1.0, 2.0, 4.0), Tolerance::DEFAULT)
        );
        assert!(AffineTransform3::try_nonuniform_scale(center, [1.0, Real::NAN, 1.0]).is_err());
        assert!(
            AffineTransform3::try_directional_scale(center, direction, Real::INFINITY).is_err()
        );
    }

    #[test]
    fn shear_uses_perpendicular_directions_and_retains_its_fixed_point() {
        let fixed = point(1.0, 2.0, 3.0);
        let reference = UnitVector3::try_new(0.0, 1.0, 0.0, Tolerance::DEFAULT).unwrap();
        let shear_direction = UnitVector3::try_new(-1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
        let transform =
            AffineTransform3::try_shear(fixed, reference, shear_direction, 0.5, Tolerance::DEFAULT)
                .unwrap();

        assert_eq!(transform.transform_point(fixed).unwrap(), fixed);
        assert_eq!(
            transform.transform_point(point(4.0, -2.0, 5.0)).unwrap(),
            point(6.0, -2.0, 5.0)
        );
        assert_eq!(
            transform.transform_point(point(-3.0, 2.0, 7.0)).unwrap(),
            point(-3.0, 2.0, 7.0)
        );

        assert!(
            AffineTransform3::try_shear(fixed, reference, reference, 0.5, Tolerance::DEFAULT,)
                .is_err()
        );
        assert!(
            AffineTransform3::try_shear(
                fixed,
                reference,
                shear_direction,
                Real::NAN,
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }

    #[test]
    fn axis_rotation_uses_rodrigues_formula_about_a_fixed_point() {
        let center = point(1.0, 1.0, 0.0);
        let axis = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let transform =
            AffineTransform3::try_rotation(center, axis, std::f64::consts::FRAC_PI_2).unwrap();
        assert!(
            transform
                .transform_point(center)
                .unwrap()
                .is_near(center, Tolerance::DEFAULT)
        );
        assert!(
            transform
                .transform_point(point(2.0, 1.0, 0.0))
                .unwrap()
                .is_near(point(1.0, 2.0, 0.0), Tolerance::DEFAULT)
        );
        assert!(AffineTransform3::try_rotation(center, axis, Real::INFINITY).is_err());
    }

    #[test]
    fn shortest_rotation_maps_parallel_oblique_and_antiparallel_directions() {
        let directions = [
            (
                UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap(),
                UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap(),
            ),
            (
                UnitVector3::try_new(1.0, 2.0, 3.0, Tolerance::DEFAULT).unwrap(),
                UnitVector3::try_new(-2.0, 4.0, 1.0, Tolerance::DEFAULT).unwrap(),
            ),
            (
                UnitVector3::try_new(0.0, 1.0, 0.0, Tolerance::DEFAULT).unwrap(),
                UnitVector3::try_new(0.0, -1.0, 0.0, Tolerance::DEFAULT).unwrap(),
            ),
            (
                UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap(),
                UnitVector3::try_new(-1.0, 1.0e-8, 0.0, Tolerance::DEFAULT).unwrap(),
            ),
        ];
        for (from, to) in directions {
            let rotation =
                AffineTransform3::try_rotation_between(from, to, Tolerance::DEFAULT).unwrap();
            let actual = rotation.transform_vector(from.as_vector()).unwrap();
            for (actual, expected) in actual.to_array().into_iter().zip(to.as_vector().to_array()) {
                assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
            }
            let rows = rotation.linear_rows();
            for row in 0..3 {
                for column in 0..3 {
                    let dot = (0..3)
                        .map(|index| rows[row][index] * rows[column][index])
                        .sum::<Real>();
                    let expected = if row == column { 1.0 } else { 0.0 };
                    assert!(Tolerance::DEFAULT.approx_eq(dot, expected));
                }
            }
        }
    }

    #[test]
    fn direction_mapping_scales_axially_or_uniformly_around_the_source_origin() {
        let source_origin = point(1.0, 2.0, 3.0);
        let target_origin = point(10.0, -1.0, 4.0);
        let source_direction = UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
        let target_direction = UnitVector3::try_new(0.0, 1.0, 0.0, Tolerance::DEFAULT).unwrap();
        let axial = AffineTransform3::try_direction_mapping(
            source_origin,
            source_direction,
            target_origin,
            target_direction,
            3.0,
            1.0,
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (source, expected) in [
            (source_origin, target_origin),
            (point(2.0, 2.0, 3.0), point(10.0, 2.0, 4.0)),
            (point(1.0, 3.0, 3.0), point(9.0, -1.0, 4.0)),
            (point(1.0, 2.0, 4.0), point(10.0, -1.0, 5.0)),
        ] {
            assert!(
                axial
                    .transform_point(source)
                    .unwrap()
                    .is_near(expected, Tolerance::DEFAULT)
            );
        }

        let uniform = AffineTransform3::try_direction_mapping(
            source_origin,
            source_direction,
            target_origin,
            target_direction,
            3.0,
            3.0,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            uniform
                .transform_point(point(1.0, 3.0, 4.0))
                .unwrap()
                .is_near(point(7.0, -1.0, 7.0), Tolerance::DEFAULT)
        );
    }

    #[test]
    fn frame_mapping_matches_origins_axes_and_uniform_scale() {
        let source = Frame3::try_from_points(
            point(1.0, 2.0, 3.0),
            point(3.0, 2.0, 3.0),
            point(1.0, 3.0, 4.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let target = Frame3::try_from_points(
            point(10.0, -1.0, 4.0),
            point(10.0, 5.0, 4.0),
            point(8.0, -1.0, 8.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let transform =
            AffineTransform3::try_frame_mapping(source, target, [3.0, 3.0, 3.0]).unwrap();
        assert!(
            transform
                .transform_point(source.origin())
                .unwrap()
                .is_near(target.origin(), Tolerance::DEFAULT)
        );
        for (source_axis, target_axis) in source.axes().into_iter().zip(target.axes()) {
            let actual = transform.transform_vector(source_axis.as_vector()).unwrap();
            let expected = target_axis.as_vector().scaled(3.0).unwrap();
            for (actual, expected) in actual.to_array().into_iter().zip(expected.to_array()) {
                assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
            }
        }
    }

    #[test]
    fn reflection_fixes_its_plane_and_reverses_the_normal_coordinate() {
        let point_on_plane = point(2.0, -5.0, 7.0);
        let normal = UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
        let transform = AffineTransform3::try_reflection(point_on_plane, normal).unwrap();
        assert_eq!(
            transform.transform_point(point_on_plane).unwrap(),
            point_on_plane
        );
        assert_eq!(
            transform.transform_point(point(5.0, 3.0, -1.0)).unwrap(),
            point(-1.0, 3.0, -1.0)
        );
        assert_eq!(
            transform
                .transform_vector(Vector3::try_new(1.0, 2.0, 3.0).unwrap())
                .unwrap(),
            Vector3::try_new(-1.0, 2.0, 3.0).unwrap()
        );
    }
}
