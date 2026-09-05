use super::*;

#[cfg(test)]
mod tests;

impl NurbsCurve {
    /// Evaluates the curve with the homogeneous de Boor algorithm.
    pub fn evaluate(&self, parameter: Real) -> Result<Point3, GeometryError> {
        let span = self.checked_span(parameter)?;
        if let Some(point) = self.span_endpoint_point(span, parameter) {
            return Ok(point);
        }
        self.with_evaluation_controls(span, |origin, work| {
            let homogeneous = de_boor(&self.knots, self.degree, span, parameter, work)?;
            self.restore_evaluated_point(span, parameter, project_homogeneous(homogeneous)?, origin)
        })
    }

    /// Evaluates the point and exact first derivative using the derivative
    /// control polygon in homogeneous coordinates and the rational quotient
    /// rule.
    pub fn evaluate_with_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point3, Vector3), GeometryError> {
        let span = self.checked_span(parameter)?;
        self.with_evaluation_controls(span, |origin, active| {
            let homogeneous = de_boor(&self.knots, self.degree, span, parameter, active.clone())?;
            let point = project_homogeneous(homogeneous)?;

            let first_control_point = span - self.degree;
            let mut derivative_controls = Vec::with_capacity(self.degree);
            for local_index in 0..self.degree {
                let control_point_index = first_control_point + local_index;
                let knot_start = self.knots[control_point_index + 1];
                let knot_end = self.knots[control_point_index + self.degree + 1];
                let mut derivative = [0.0; 4];
                for coordinate in 0..4 {
                    derivative[coordinate] = stable_divided_difference(
                        active[local_index + 1][coordinate],
                        active[local_index][coordinate],
                        self.degree,
                        knot_start,
                        knot_end,
                    )?;
                }
                derivative_controls.push(derivative);
            }

            let homogeneous_derivative = de_boor(
                &self.knots[1..self.knots.len() - 1],
                self.degree - 1,
                span - 1,
                parameter,
                derivative_controls,
            )?;
            let weight = homogeneous[3];
            let weight_derivative = homogeneous_derivative[3];
            let point_coordinates = point.to_array();
            let derivative: [Real; 3] = std::array::from_fn(|coordinate| {
                (-point_coordinates[coordinate])
                    .mul_add(weight_derivative, homogeneous_derivative[coordinate])
                    / weight
            });
            Ok((
                self.restore_evaluated_point(span, parameter, point, origin)?,
                Vector3::try_from(derivative)?,
            ))
        })
    }

    /// Evaluates the point and exact first and second derivatives using
    /// homogeneous derivative control polygons and the rational quotient
    /// rule.
    pub fn evaluate_with_second_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let span = self.checked_span(parameter)?;
        self.with_evaluation_controls(span, |origin, active| {
            let homogeneous = de_boor(&self.knots, self.degree, span, parameter, active.clone())?;
            let point = project_homogeneous(homogeneous)?;

            let first_control_point = span - self.degree;
            let mut derivative_controls = Vec::with_capacity(self.degree);
            for local_index in 0..self.degree {
                let control_point_index = first_control_point + local_index;
                let knot_start = self.knots[control_point_index + 1];
                let knot_end = self.knots[control_point_index + self.degree + 1];
                let mut derivative = [0.0; 4];
                for coordinate in 0..4 {
                    derivative[coordinate] = stable_divided_difference(
                        active[local_index + 1][coordinate],
                        active[local_index][coordinate],
                        self.degree,
                        knot_start,
                        knot_end,
                    )?;
                }
                derivative_controls.push(derivative);
            }

            let homogeneous_derivative = de_boor(
                &self.knots[1..self.knots.len() - 1],
                self.degree - 1,
                span - 1,
                parameter,
                derivative_controls.clone(),
            )?;
            let weight = homogeneous[3];
            let weight_derivative = homogeneous_derivative[3];
            let point_coordinates = point.to_array();
            let first_derivative: [Real; 3] = std::array::from_fn(|coordinate| {
                (-point_coordinates[coordinate])
                    .mul_add(weight_derivative, homogeneous_derivative[coordinate])
                    / weight
            });
            let first_derivative = Vector3::try_from(first_derivative)?;

            if self.degree == 1 {
                // A degree-one homogeneous curve has H'' = 0, but its
                // Euclidean second derivative is -2 (W'/W) C', not zero.
                let second = first_derivative.to_array().map(|value| {
                    crate::parameter::scaled_ratio(value, weight_derivative, weight)
                        .map(|value| -2.0 * value)
                });
                let [x, y, z] = second;
                return Ok((
                    self.restore_evaluated_point(span, parameter, point, origin)?,
                    first_derivative,
                    Vector3::try_new(x?, y?, z?)?,
                ));
            }

            let mut second_derivative_controls = Vec::with_capacity(self.degree - 1);
            for local_index in 0..self.degree - 1 {
                let derivative_control_index = first_control_point + local_index;
                let knot_start = self.knots[derivative_control_index + 2];
                let knot_end = self.knots[derivative_control_index + self.degree + 1];
                let mut derivative = [0.0; 4];
                for coordinate in 0..4 {
                    derivative[coordinate] = stable_divided_difference(
                        derivative_controls[local_index + 1][coordinate],
                        derivative_controls[local_index][coordinate],
                        self.degree - 1,
                        knot_start,
                        knot_end,
                    )?;
                }
                second_derivative_controls.push(derivative);
            }
            let homogeneous_second_derivative = de_boor(
                &self.knots[2..self.knots.len() - 2],
                self.degree - 2,
                span - 2,
                parameter,
                second_derivative_controls,
            )?;
            let weight_second_derivative = homogeneous_second_derivative[3];
            let first_coordinates = first_derivative.to_array();
            let second_derivative: [Real; 3] = std::array::from_fn(|coordinate| {
                let quotient_terms = (2.0 * weight_derivative).mul_add(
                    first_coordinates[coordinate],
                    weight_second_derivative * point_coordinates[coordinate],
                );
                (homogeneous_second_derivative[coordinate] - quotient_terms) / weight
            });
            Ok((
                self.restore_evaluated_point(span, parameter, point, origin)?,
                first_derivative,
                Vector3::try_from(second_derivative)?,
            ))
        })
    }

    fn span_endpoint_point(&self, span: usize, parameter: Real) -> Option<Point3> {
        if parameter == self.knots[span]
            && self.knots[span + 1 - self.degree..=span]
                .iter()
                .all(|knot| *knot == parameter)
        {
            return Some(self.control_points[span - self.degree].point);
        }
        if parameter == self.knots[span + 1]
            && self.knots[span + 1..=span + self.degree]
                .iter()
                .all(|knot| *knot == parameter)
        {
            return Some(self.control_points[span].point);
        }
        None
    }

    fn restore_evaluated_point(
        &self,
        span: usize,
        parameter: Real,
        point: Point3,
        origin: Point3,
    ) -> Result<Point3, GeometryError> {
        if let Some(point) = self.span_endpoint_point(span, parameter) {
            return Ok(point);
        }
        restore_origin(point, origin)
    }

    fn with_evaluation_controls<T>(
        &self,
        span: usize,
        evaluate: impl Fn(Point3, Vec<[Real; 4]>) -> Result<T, GeometryError>,
    ) -> Result<T, GeometryError> {
        let (origin, controls) = self.homogeneous_controls(span, true)?;
        let result = evaluate(origin, controls);
        // Signed-weight curves can leave their control hull. A local offset
        // may overflow even when the final world-space point remains finite.
        // Retry that exceptional case in the unshifted frame.
        if matches!(result, Err(GeometryError::NonFinite { .. })) && origin.to_array() != [0.0; 3] {
            let (origin, controls) = self.homogeneous_controls(span, false)?;
            evaluate(origin, controls)
        } else {
            result
        }
    }

    fn homogeneous_controls(
        &self,
        span: usize,
        center: bool,
    ) -> Result<(Point3, Vec<[Real; 4]>), GeometryError> {
        let active = &self.control_points[span - self.degree..=span];
        let candidate = active[0].point;
        // Center local coordinates before the rational quotient rule. This
        // removes cancellation between H' and C W' under large translations.
        // A control hull wider than f64's range uses the original coordinates.
        let origin = if center
            && active.iter().all(|c| {
                c.point
                    .to_array()
                    .into_iter()
                    .zip(candidate.to_array())
                    .all(|(a, b)| (a - b).is_finite())
            }) {
            candidate
        } else {
            Point3::try_new(0.0, 0.0, 0.0)?
        };
        let weight_scale = active.iter().map(|c| c.weight.abs()).fold(0.0, Real::max);
        let mut controls = Vec::with_capacity(active.len());
        for control in active {
            let weight = control.weight / weight_scale;
            let point = control.point.to_array();
            let origin = origin.to_array();
            let value = [
                (point[0] - origin[0]) * weight,
                (point[1] - origin[1]) * weight,
                (point[2] - origin[2]) * weight,
                weight,
            ];
            require_finite(value, "local homogeneous NURBS control point")?;
            controls.push(value);
        }
        Ok((origin, controls))
    }
}

fn restore_origin(point: Point3, origin: Point3) -> Result<Point3, GeometryError> {
    Point3::try_new(
        point.x() + origin.x(),
        point.y() + origin.y(),
        point.z() + origin.z(),
    )
}
