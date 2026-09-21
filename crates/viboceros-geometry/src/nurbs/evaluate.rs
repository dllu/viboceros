use super::*;
use crate::{ParameterSide, UnitVector3};

mod exact;
mod jet;
mod query;
mod scaled_first;
mod tangent;
use jet::FloatJet;
pub(super) use query::CurveQuery;
type CurveJet = (Point3, Vector3, Vector3);

struct EvaluationControls {
    origin: Point3,
    homogeneous: Vec<[Real; 4]>,
    needs_exact: bool,
}

#[cfg(test)]
mod tests;

impl NurbsCurve {
    /// Evaluates the curve with the homogeneous de Boor algorithm.
    pub fn evaluate(&self, parameter: Real) -> Result<Point3, GeometryError> {
        self.evaluate_on_side(parameter, ParameterSide::Right)
    }

    /// Exact point limit from the requested side of a knot.
    pub fn evaluate_on_side(
        &self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<Point3, GeometryError> {
        let span = self.checked_span_on_side(parameter, side)?;
        if let Some(point) = self.span_endpoint_point(span, parameter) {
            return Ok(point);
        }
        self.with_evaluation_controls(
            span,
            |origin, work| {
                let homogeneous = de_boor(&self.knots, self.degree, span, parameter, work)?;
                self.restore_evaluated_point(
                    span,
                    parameter,
                    project_homogeneous(homogeneous)?,
                    origin,
                )
            },
            || Ok(self.exact_jet(span, parameter, 0)?.0),
        )
    }

    /// Evaluates the point and exact first derivative using the derivative
    /// control polygon in homogeneous coordinates and the rational quotient
    /// rule.
    pub fn evaluate_with_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point3, Vector3), GeometryError> {
        self.evaluate_with_derivative_on_side(parameter, ParameterSide::Right)
    }

    pub fn evaluate_with_derivative_on_side(
        &self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<(Point3, Vector3), GeometryError> {
        let (point, first, _) = self.jet_on_side(parameter, side, 1)?;
        Ok((point, first))
    }

    /// Evaluates the point and exact first and second derivatives using
    /// homogeneous derivative control polygons and the rational quotient
    /// rule.
    pub fn evaluate_with_second_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        self.evaluate_with_second_derivative_on_side(parameter, ParameterSide::Right)
    }

    pub fn evaluate_with_second_derivative_on_side(
        &self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<CurveJet, GeometryError> {
        self.jet_on_side(parameter, side, 2)
    }

    fn jet_on_side(
        &self,
        parameter: Real,
        side: ParameterSide,
        order: u8,
    ) -> Result<CurveJet, GeometryError> {
        let span = self.checked_span_on_side(parameter, side)?;
        self.with_evaluation_controls(
            span,
            |origin, active| FloatJet::new(origin, active).evaluate(self, span, parameter, order),
            || self.exact_jet(span, parameter, order),
        )
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
        side: ParameterSide,
    ) -> Result<usize, GeometryError> {
        self.validate_parameter(parameter)?;
        let domain = self.domain();
        if side == ParameterSide::Left && parameter > *domain.start() && parameter < *domain.end() {
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
        exact: impl Fn() -> Result<T, GeometryError>,
    ) -> Result<T, GeometryError> {
        let controls = self.homogeneous_controls(span, true)?;
        if controls.needs_exact {
            return exact();
        }
        let origin = controls.origin;
        let result = evaluate(origin, controls.homogeneous);
        // Signed-weight curves can leave their control hull. A local offset
        // may overflow even when the final world-space point remains finite.
        // Retry that exceptional case in the unshifted frame.
        if matches!(result, Err(GeometryError::NonFinite { .. })) && origin.to_array() != [0.0; 3] {
            let controls = self.homogeneous_controls(span, false)?;
            if controls.needs_exact {
                exact()
            } else {
                evaluate(controls.origin, controls.homogeneous).or_else(|_| exact())
            }
        } else {
            // Loss can also arise later in the recurrence or quotient rule.
            // A reported failure is not proof of a pole or nonrepresentability.
            result.or_else(|_| exact())
        }
    }

    fn homogeneous_controls(
        &self,
        span: usize,
        center: bool,
    ) -> Result<EvaluationControls, GeometryError> {
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
        let mut needs_exact = false;
        let negative_weight = active[0].weight.is_sign_negative();
        for control in active {
            let weight = control.weight / weight_scale;
            // Mixed-sign weights can have a true pole even when a rounded
            // blend ratio produces a small nonzero denominator. No numerical
            // tolerance can certify that cancellation; keep the span exact.
            needs_exact |=
                !weight.is_normal() || control.weight.is_sign_negative() != negative_weight;
            let point = control.point.to_array();
            let origin = origin.to_array();
            let local: [Real; 3] = std::array::from_fn(|i| point[i] - origin[i]);
            let value = [
                local[0] * weight,
                local[1] * weight,
                local[2] * weight,
                weight,
            ];
            needs_exact |= local
                .into_iter()
                .zip(value)
                .any(|(a, product)| a != 0. && !product.is_normal());
            require_finite(value, "local homogeneous NURBS control point")?;
            controls.push(value);
        }
        Ok(EvaluationControls {
            origin,
            homogeneous: controls,
            needs_exact,
        })
    }
}

fn restore_origin(point: Point3, origin: Point3) -> Result<Point3, GeometryError> {
    Point3::try_new(
        point.x() + origin.x(),
        point.y() + origin.y(),
        point.z() + origin.z(),
    )
}
