//! Exact and cubic deformable revolution control nets.
use super::*;
#[cfg(test)]
mod tests;

impl NurbsSurface {
    /// Cubic approximation in U with the original rational profile in V.
    /// Full turns use a periodic angular basis (`point_count + 3` controls).
    /// Partial turns use a clamped basis with `point_count` controls and
    /// tangent-aligned end handles. U retains the exact revolution's domain.
    pub fn try_deformable_revolved_curve(
        curve: &NurbsCurve,
        origin: Point3,
        axis: UnitVector3,
        start: Real,
        sweep: Real,
        point_count: usize,
    ) -> Result<Self, GeometryError> {
        require_finite([start], "revolution start angle")?;
        if !sweep.is_finite() || sweep == 0. || sweep.abs() > std::f64::consts::TAU {
            return Err(GeometryError::InvalidRevolutionSweep);
        }
        if !(4..=crate::MAX_CURVE_REBUILD_POINT_COUNT).contains(&point_count) {
            return Err(GeometryError::InvalidCurveRebuildPointCount {
                actual: point_count,
                minimum: 4,
                maximum: crate::MAX_CURVE_REBUILD_POINT_COUNT,
            });
        }
        let length = curve.revolution_parameter_length(origin, axis, sweep)?;
        let zero = Point3::try_new(0., 0., 0.)?;
        let circle = crate::Circle3::try_from_frame(
            zero,
            1.,
            UnitVector3::try_new(1., 0., 0., Tolerance::DEFAULT)?,
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT)?,
            Tolerance::DEFAULT,
        )?;
        let angular = crate::CircularArc3::try_from_circle_sweep(circle, sweep.abs())?
            .try_deformable(3, point_count)?
            .try_reparameterized(0.0..=length)?;
        let chart_start = if sweep < 0. { start + sweep } else { start };
        let rotation = AffineTransform3::try_rotation(
            zero,
            axis,
            chart_start.rem_euclid(std::f64::consts::TAU),
        )?;
        let axis_vector = axis.as_vector();
        let nu = angular.control_points().len();
        let nv = curve.control_points().len();
        let count = nu.checked_mul(nv).ok_or(GeometryError::InvalidControlNet {
            context: "deformable revolution control-net size overflowed usize",
        })?;
        let mut controls = Vec::new();
        controls
            .try_reserve_exact(count)
            .map_err(|_| GeometryError::InvalidControlNet {
                context: "deformable revolution control net exceeds addressable memory",
            })?;
        for cv in curve.control_points() {
            let offset = origin.vector_to(cv.point())?;
            let axial = axis_vector.scaled(offset.dot(axis_vector)?)?;
            let radial = Vector3::try_new(
                offset.x() - axial.x(),
                offset.y() - axial.y(),
                offset.z() - axial.z(),
            )?;
            let radial = rotation.transform_vector(radial)?;
            let tangent = axis_vector.cross(radial)?;
            let center = origin.translated(axial)?;
            for angle in angular.control_points() {
                let p = center
                    .translated(radial.scaled(angle.point().x())?)?
                    .translated(tangent.scaled(angle.point().y())?)?;
                controls.push(WeightedPoint3::try_new(p, cv.weight())?);
            }
        }
        Self::try_new_rational(
            3,
            curve.degree(),
            nu,
            nv,
            controls,
            angular.knots().to_vec(),
            curve.knots().to_vec(),
        )
    }

    /// Constructs an exact rational surface by revolving a NURBS profile.
    ///
    /// U is the quadratic rational revolution direction and V preserves the
    /// profile's degree and complete knot vector. The U domain follows Rhino's
    /// exact-revolve convention: sweep radians multiplied by the profile's
    /// OpenNURBS 65-sample maximum-radius estimate. Quadrant knots are fully
    /// multiple and every surface weight is the product of its angular and
    /// profile-curve weights.
    pub fn try_revolved_curve(
        curve: &crate::NurbsCurve,
        axis_origin: Point3,
        axis_direction: UnitVector3,
        start_angle_radians: Real,
        sweep_angle_radians: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([start_angle_radians], "revolution start angle")?;
        if !sweep_angle_radians.is_finite()
            || sweep_angle_radians == 0.0
            || sweep_angle_radians.abs() > std::f64::consts::TAU
        {
            return Err(GeometryError::InvalidRevolutionSweep);
        }

        let axis_vector = axis_direction.as_vector();
        let axis_center = |point: Point3| -> Result<Point3, GeometryError> {
            let offset = axis_origin.vector_to(point)?;
            axis_origin.translated(axis_vector.scaled(offset.dot(axis_vector)?)?)
        };
        let domain_length =
            curve.revolution_parameter_length(axis_origin, axis_direction, sweep_angle_radians)?;

        let sweep_magnitude = sweep_angle_radians.abs();
        let quadrant_limit = (0.5 + f64::EPSILON.sqrt()) * std::f64::consts::PI;
        let span_count: usize = if sweep_magnitude <= quadrant_limit {
            1
        } else if sweep_magnitude <= 2.0 * quadrant_limit {
            2
        } else {
            4
        };
        let segment_angle = sweep_angle_radians / span_count as Real;
        let middle_weight = (0.5 * segment_angle).cos();
        let control_count_u = span_count
            .checked_mul(2)
            .and_then(|count| count.checked_add(1))
            .ok_or(GeometryError::InvalidControlNet {
                context: "revolution control-point count overflowed usize",
            })?;
        let control_count_v = curve.control_points().len();
        let control_count = control_count_u.checked_mul(control_count_v).ok_or(
            GeometryError::InvalidControlNet {
                context: "revolution control-net size overflowed usize",
            },
        )?;
        let mut controls = Vec::new();
        controls.try_reserve_exact(control_count).map_err(|_| {
            GeometryError::InvalidControlNet {
                context: "revolution control net exceeds addressable memory",
            }
        })?;

        let normalized_start = start_angle_radians.rem_euclid(std::f64::consts::TAU);
        let full_turn = sweep_magnitude == std::f64::consts::TAU;
        for profile_control in curve.control_points() {
            let point = profile_control.point();
            let center = axis_center(point)?;
            let first =
                AffineTransform3::try_rotation(axis_origin, axis_direction, normalized_start)?
                    .transform_point(point)?;
            controls.push(WeightedPoint3::try_new(first, profile_control.weight())?);
            for span in 0..span_count {
                let middle_angle = normalized_start + (span as Real + 0.5) * segment_angle;
                let rotated_middle =
                    AffineTransform3::try_rotation(axis_origin, axis_direction, middle_angle)?
                        .transform_point(point)?;
                let middle = center.translated(
                    center
                        .vector_to(rotated_middle)?
                        .scaled(middle_weight.recip())?,
                )?;
                controls.push(WeightedPoint3::try_new(
                    middle,
                    profile_control.weight() * middle_weight,
                )?);

                let endpoint = if full_turn && span + 1 == span_count {
                    first
                } else {
                    AffineTransform3::try_rotation(
                        axis_origin,
                        axis_direction,
                        normalized_start + (span + 1) as Real * segment_angle,
                    )?
                    .transform_point(point)?
                };
                controls.push(WeightedPoint3::try_new(endpoint, profile_control.weight())?);
            }
        }

        let mut knots_u = Vec::with_capacity(control_count_u + 3);
        knots_u.extend([0.0; 3]);
        for boundary in 1..span_count {
            let knot = domain_length * boundary as Real / span_count as Real;
            knots_u.extend([knot; 2]);
        }
        knots_u.extend([domain_length; 3]);
        Self::try_new_rational(
            2,
            curve.degree(),
            control_count_u,
            control_count_v,
            controls,
            knots_u,
            curve.knots().to_vec(),
        )
    }
}
