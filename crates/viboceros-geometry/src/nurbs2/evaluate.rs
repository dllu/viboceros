//! Stable native-parameter evaluation of UV trims, including exact knot sides.

use super::*;
use crate::ParameterSide;
use crate::nurbs::{de_boor, find_span_in_knots, stable_divided_difference};

#[cfg(test)]
mod tests;

impl NurbsCurve2 {
    pub fn evaluate(&self, parameter: Real) -> Result<Point2, GeometryError> {
        self.evaluate_on_side(parameter, ParameterSide::Right)
    }

    /// Exact point limit at a knot; domain endpoints use the interior span.
    pub fn evaluate_on_side(
        &self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<Point2, GeometryError> {
        let span = self.checked_span(parameter, side)?;
        if let Some(point) = self.endpoint(span, parameter) {
            return Ok(point);
        }
        self.with_controls(span, |origin, active| {
            let h = de_boor(&self.knots, self.degree, span, parameter, active)?;
            self.restore(span, parameter, project(h)?, origin)
        })
    }

    pub fn evaluate_with_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point2, [Real; 2]), GeometryError> {
        self.evaluate_with_derivative_on_side(parameter, ParameterSide::Right)
    }

    /// Point and native first derivative on one side of a knot.
    pub fn evaluate_with_derivative_on_side(
        &self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<(Point2, [Real; 2]), GeometryError> {
        let span = self.checked_span(parameter, side)?;
        self.with_controls(span, |origin, active| {
            let h = de_boor(&self.knots, self.degree, span, parameter, active.clone())?;
            let point = project(h)?;
            let first = span - self.degree;
            let derivatives = (0..self.degree)
                .map(|i| {
                    let mut derivative = [0.0; 3];
                    for coordinate in 0..3 {
                        derivative[coordinate] = stable_divided_difference(
                            active[i + 1][coordinate],
                            active[i][coordinate],
                            self.degree,
                            self.knots[first + i + 1],
                            self.knots[first + i + self.degree + 1],
                        )?;
                    }
                    Ok(derivative)
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            let dh = de_boor(
                &self.knots[1..self.knots.len() - 1],
                self.degree - 1,
                span - 1,
                parameter,
                derivatives,
            )?;
            let derivative = [
                (-point.x()).mul_add(dh[2], dh[0]) / h[2],
                (-point.y()).mul_add(dh[2], dh[1]) / h[2],
            ];
            require_finite(derivative, "parameter-space NURBS derivative")?;
            Ok((self.restore(span, parameter, point, origin)?, derivative))
        })
    }

    fn checked_span(&self, parameter: Real, side: ParameterSide) -> Result<usize, GeometryError> {
        require_finite([parameter], "parameter-space NURBS parameter")?;
        let domain = self.domain();
        let (domain_start, domain_end) = (*domain.start(), *domain.end());
        if parameter < domain_start || parameter > domain_end {
            return Err(GeometryError::ParameterOutOfDomain {
                parameter,
                domain_start,
                domain_end,
            });
        }
        if side == ParameterSide::Left && parameter > domain_start && parameter < domain_end {
            Ok(self.knots.partition_point(|k| *k < parameter) - 1)
        } else {
            Ok(find_span_in_knots(
                &self.knots,
                self.degree,
                self.control_points.len(),
                parameter,
            ))
        }
    }

    fn endpoint(&self, span: usize, parameter: Real) -> Option<Point2> {
        if parameter == self.knots[span]
            && self.knots[span + 1 - self.degree..=span]
                .iter()
                .all(|k| *k == parameter)
        {
            return Some(self.control_points[span - self.degree].point);
        }
        if parameter == self.knots[span + 1]
            && self.knots[span + 1..=span + self.degree]
                .iter()
                .all(|k| *k == parameter)
        {
            return Some(self.control_points[span].point);
        }
        None
    }

    fn restore(
        &self,
        span: usize,
        parameter: Real,
        point: Point2,
        origin: Point2,
    ) -> Result<Point2, GeometryError> {
        if let Some(point) = self.endpoint(span, parameter) {
            return Ok(point);
        }
        Point2::try_new(point.x() + origin.x(), point.y() + origin.y())
    }

    fn with_controls<T>(
        &self,
        span: usize,
        evaluate: impl Fn(Point2, Vec<[Real; 3]>) -> Result<T, GeometryError>,
    ) -> Result<T, GeometryError> {
        let (origin, controls) = self.controls(span, true)?;
        let result = evaluate(origin, controls);
        // A signed rational image can leave the hull. A centered result may
        // overflow even when the final UV point is finite; retry uncentered.
        if matches!(result, Err(GeometryError::NonFinite { .. })) && origin.to_array() != [0.0; 2] {
            let (origin, controls) = self.controls(span, false)?;
            evaluate(origin, controls)
        } else {
            result
        }
    }

    fn controls(
        &self,
        span: usize,
        center: bool,
    ) -> Result<(Point2, Vec<[Real; 3]>), GeometryError> {
        let active = &self.control_points[span - self.degree..=span];
        let candidate = active[0].point;
        // Choose each coordinate independently. Even the uncentered retry
        // keeps constant coordinates centered, so overflow in U cannot move
        // an exact V boundary (or vice versa).
        let origin = Point2::try_from(std::array::from_fn(|axis| {
            let value = candidate.to_array()[axis];
            if active.iter().all(|c| c.point.to_array()[axis] == value)
                || center
                    && active
                        .iter()
                        .all(|c| (c.point.to_array()[axis] - value).is_finite())
            {
                value
            } else {
                0.0
            }
        }))?;
        let scale = active.iter().map(|c| c.weight.abs()).fold(0.0, Real::max);
        let controls = active
            .iter()
            .map(|c| {
                let w = c.weight / scale;
                let h = [
                    (c.point.x() - origin.x()) * w,
                    (c.point.y() - origin.y()) * w,
                    w,
                ];
                require_finite(h, "local homogeneous parameter-space NURBS control")?;
                Ok(h)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok((origin, controls))
    }
}

fn project(h: [Real; 3]) -> Result<Point2, GeometryError> {
    if h[2] == 0.0 || !h[2].is_finite() {
        return Err(GeometryError::ZeroWeightAtParameter);
    }
    Point2::try_new(h[0] / h[2], h[1] / h[2])
}
