use nalgebra::Matrix3;

use crate::{Frame3, GeometryError, Plane, Point3, Real, UnitVector3, Vector3, require_finite};

/// A finite affine map from three-dimensional model space to itself.
///
/// The linear part is stored with nalgebra, while application uses the
/// kernel's compensated dot products and exact fallbacks. Point application
/// includes translation in each sum, before rounding or overflow validation.
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

    /// Orthogonally projects points onto a plane while retaining components
    /// tangent to it. The result is intentionally singular.
    pub fn try_planar_projection(plane: Plane) -> Result<Self, GeometryError> {
        Self::try_directional_scale(plane.origin(), plane.normal(), 0.0)
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
            linear: Matrix3::from_fn(|row, column| linear_rows[row][column]),
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
        let point = Vector3::try_from(point.to_array())?;
        let rows = self.linear_rows();
        let translation = self.translation.to_array();
        let component = |i| point.dot_with_offset(Vector3::try_from(rows[i])?, translation[i]);
        Point3::try_new(component(0)?, component(1)?, component(2)?)
    }

    pub fn transform_vector(self, vector: Vector3) -> Result<Vector3, GeometryError> {
        Vector3::try_from(self.linear_coordinates(vector)?)
    }

    /// Composes maps in application order: `self`, followed by `next`.
    /// Singular maps are valid. Rejects unrepresentable composed coefficients;
    /// rounded composition need not be bit-identical to sequential evaluation.
    pub fn then(self, next: Self) -> Result<Self, GeometryError> {
        let linear = multiply_linear(next.linear_rows(), self.linear_rows())?;
        let translation = next.transform_point(Point3::try_from(self.translation.to_array())?)?;
        Self::try_new(linear, Vector3::try_from(translation.to_array())?)
    }

    /// Returns whether the linear part reverses orientation. Singular maps do
    /// not have a well-defined orientation and are rejected.
    pub(crate) fn orientation_reversing(self) -> Result<bool, GeometryError> {
        let rows = self.linear_rows();
        let scale = rows
            .iter()
            .flatten()
            .map(|value| value.abs())
            .fold(0.0, Real::max);
        if scale == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "affine transform linear part",
            });
        }
        let matrix = Matrix3::from_fn(|row, column| rows[row][column] / scale);
        let determinant = matrix.determinant();
        if !determinant.is_finite() || determinant == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "affine transform linear part",
            });
        }
        Ok(determinant < 0.0)
    }

    /// Conservative upper bound on the linear part's maximum singular value.
    /// This is used to safely propagate model-space component tolerances.
    pub(crate) fn maximum_linear_scale(self) -> Result<Real, GeometryError> {
        let rows = self.linear_rows();
        let scale = rows
            .iter()
            .flatten()
            .map(|value| value.abs())
            .fold(0.0, Real::max);
        if scale == 0.0 {
            return Ok(0.0);
        }
        let normalized = rows.map(|row| row.map(|value| value.abs() / scale));
        let maximum_row_sum = normalized
            .iter()
            .map(|row| row.iter().sum::<Real>())
            .fold(0.0, Real::max);
        let maximum_column_sum = (0..3)
            .map(|column| normalized.iter().map(|row| row[column]).sum::<Real>())
            .fold(0.0, Real::max);
        // ||A||_2 <= sqrt(||A||_1 ||A||_infinity). Scaling first keeps both
        // induced norms finite, while this bound remains exact for identity
        // and diagonal scale transforms.
        let maximum = scale * (maximum_row_sum * maximum_column_sum).sqrt();
        require_finite([maximum], "affine transform scale bound")?;
        Ok(maximum)
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
        Self::try_mapping_origins(linear_rows, fixed_point, fixed_point)
    }

    fn try_mapping_origins(
        linear_rows: [[Real; 3]; 3],
        source_origin: Point3,
        target_origin: Point3,
    ) -> Result<Self, GeometryError> {
        // Translation is target - A*source. Include target in each sum;
        // A*source alone can overflow or lose a contribution to cancellation.
        let source = Vector3::try_from(source_origin.to_array())?;
        let target = target_origin.to_array();
        let component = |i: usize| {
            source.dot_with_offset(
                Vector3::try_from(linear_rows[i].map(|coefficient| -coefficient))?,
                target[i],
            )
        };
        let translation = Vector3::try_new(component(0)?, component(1)?, component(2)?)?;
        Self::try_new(linear_rows, translation)
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
mod tests;
