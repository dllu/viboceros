use super::*;
use crate::{CurveEvaluationSide, UnitVector3};

#[cfg(test)]
mod tests;

impl NurbsCurve {
    /// Evaluates the curve with the homogeneous de Boor algorithm.
    pub fn evaluate(&self, parameter: Real) -> Result<Point3, GeometryError> {
        self.evaluate_on_side(parameter, CurveEvaluationSide::Right)
    }

    /// Exact point limit from the requested side of a knot.
    pub fn evaluate_on_side(
        &self,
        parameter: Real,
        side: CurveEvaluationSide,
    ) -> Result<Point3, GeometryError> {
        let span = self.checked_span_on_side(parameter, side)?;
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
        self.evaluate_with_derivative_on_side(parameter, CurveEvaluationSide::Right)
    }

    pub fn evaluate_with_derivative_on_side(
        &self,
        parameter: Real,
        side: CurveEvaluationSide,
    ) -> Result<(Point3, Vector3), GeometryError> {
        let span = self.checked_span_on_side(parameter, side)?;
        self.with_evaluation_controls(span, |origin, active| {
            let homogeneous = de_boor(&self.knots, self.degree, span, parameter, active.clone())?;
            let point = project_homogeneous(homogeneous)?;

            let derivative_controls = self.derivative_controls(span, 1, &active)?;

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
        self.evaluate_with_second_derivative_on_side(parameter, CurveEvaluationSide::Right)
    }

    pub fn evaluate_with_second_derivative_on_side(
        &self,
        parameter: Real,
        side: CurveEvaluationSide,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let span = self.checked_span_on_side(parameter, side)?;
        self.with_evaluation_controls(span, |origin, active| {
            let homogeneous = de_boor(&self.knots, self.degree, span, parameter, active.clone())?;
            let point = project_homogeneous(homogeneous)?;

            let derivative_controls = self.derivative_controls(span, 1, &active)?;

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

            let second_derivative_controls =
                self.derivative_controls(span, 2, &derivative_controls)?;
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

    /// The oriented limiting tangent, including stationary points with a
    /// nonzero higher derivative. A locally constant span has no tangent.
    pub fn tangent_at_on_side(
        &self,
        parameter: Real,
        side: CurveEvaluationSide,
    ) -> Result<UnitVector3, GeometryError> {
        let (_, first) = self.evaluate_with_derivative_on_side(parameter, side)?;
        if first.to_array() != [0.0; 3] {
            return first.normalized_nonzero();
        }
        let span = self.checked_span_on_side(parameter, side)?;
        let domain = self.domain();
        let incoming = parameter == *domain.end()
            || (side == CurveEvaluationSide::Left && parameter > *domain.start());
        self.with_evaluation_controls(span, |_, active| {
            let homogeneous = de_boor(&self.knots, self.degree, span, parameter, active.clone())?;
            let point = project_homogeneous(homogeneous)?.to_array();
            let mut controls = active;
            for order in 1..=self.degree {
                controls = self.derivative_controls(span, order, &controls)?;
                let derivative = de_boor(
                    &self.knots[order..self.knots.len() - order],
                    self.degree - order,
                    span - order,
                    parameter,
                    controls.clone(),
                )?;
                // If C', ..., C^(order-1) vanish, the quotient rule reduces
                // to (H^(order) - C W^(order))/W. Only direction is needed.
                let coordinates =
                    std::array::from_fn(|i| (-point[i]).mul_add(derivative[3], derivative[i]));
                let direction = Vector3::try_from(coordinates)?;
                if direction.to_array() != [0.0; 3] {
                    let sign = homogeneous[3].signum()
                        * if incoming && order % 2 == 0 {
                            -1.0
                        } else {
                            1.0
                        };
                    return direction.scaled(sign)?.normalized_nonzero();
                }
            }
            Err(GeometryError::Degenerate {
                context: "locally constant NURBS tangent",
            })
        })
    }

    fn derivative_controls(
        &self,
        span: usize,
        order: usize,
        previous: &[[Real; 4]],
    ) -> Result<Vec<[Real; 4]>, GeometryError> {
        let degree = self.degree + 1 - order;
        let first_control_point = span - self.degree;
        (0..degree)
            .map(|i| {
                let start = self.knots[first_control_point + i + order];
                let end = self.knots[first_control_point + i + self.degree + 1];
                let mut result = [0.0; 4];
                for coordinate in 0..4 {
                    result[coordinate] = stable_divided_difference(
                        previous[i + 1][coordinate],
                        previous[i][coordinate],
                        degree,
                        start,
                        end,
                    )?;
                }
                Ok(result)
            })
            .collect()
    }

    fn checked_span_on_side(
        &self,
        parameter: Real,
        side: CurveEvaluationSide,
    ) -> Result<usize, GeometryError> {
        self.validate_parameter(parameter)?;
        let domain = self.domain();
        if side == CurveEvaluationSide::Left
            && parameter > *domain.start()
            && parameter < *domain.end()
        {
            Ok(self.knots.partition_point(|knot| *knot < parameter) - 1)
        } else {
            Ok(self.find_span(parameter))
        }
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
