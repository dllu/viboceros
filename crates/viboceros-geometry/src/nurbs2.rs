use std::ops::RangeInclusive;

use crate::nurbs::{de_boor, find_span_in_knots, stable_divided_difference, validate_direction};
use crate::{GeometryError, Point2, Real, require_finite};

/// A two-dimensional Euclidean control point with a finite, nonzero weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedPoint2 {
    point: Point2,
    weight: Real,
}

impl WeightedPoint2 {
    pub fn try_new(point: Point2, weight: Real) -> Result<Self, GeometryError> {
        if weight.is_finite() && weight != 0.0 {
            Ok(Self { point, weight })
        } else {
            Err(GeometryError::InvalidWeight { index: 0 })
        }
    }

    #[inline]
    pub const fn point(self) -> Point2 {
        self.point
    }

    #[inline]
    pub const fn weight(self) -> Real {
        self.weight
    }
}

/// A finite rational B-spline curve in a surface's parameter space.
///
/// The full knot-vector convention matches [`crate::NurbsCurve`]. A separate
/// 2D type is essential for B-rep trims: its coordinates are `(u, v)`, not
/// model-space `(x, y, z)` coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve2 {
    degree: usize,
    control_points: Vec<WeightedPoint2>,
    knots: Vec<Real>,
    rational: bool,
}

impl NurbsCurve2 {
    pub fn try_new(
        degree: usize,
        control_points: Vec<Point2>,
        knots: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        let control_points = control_points
            .into_iter()
            .map(|point| WeightedPoint2 { point, weight: 1.0 })
            .collect();
        Self::try_new_rational(degree, control_points, knots)
    }

    pub fn try_new_rational(
        degree: usize,
        control_points: Vec<WeightedPoint2>,
        knots: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        validate_direction(degree, control_points.len(), &knots)?;
        for (index, control_point) in control_points.iter().enumerate() {
            if !control_point.weight.is_finite() || control_point.weight == 0.0 {
                return Err(GeometryError::InvalidWeight { index });
            }
        }
        let first_weight = control_points[0].weight;
        let rational = control_points
            .iter()
            .any(|control_point| control_point.weight != first_weight);
        Ok(Self {
            degree,
            control_points,
            knots,
            rational,
        })
    }

    /// Constructs a degree-one trim with a normalized parameter domain.
    pub fn try_line(start: Point2, end: Point2) -> Result<Self, GeometryError> {
        if start == end {
            return Err(GeometryError::Degenerate {
                context: "parameter-space line",
            });
        }
        Self::try_new(1, vec![start, end], vec![0.0, 0.0, 1.0, 1.0])
    }

    #[inline]
    pub const fn degree(&self) -> usize {
        self.degree
    }

    #[inline]
    pub fn control_points(&self) -> &[WeightedPoint2] {
        &self.control_points
    }

    #[inline]
    pub fn knots(&self) -> &[Real] {
        &self.knots
    }

    #[inline]
    pub const fn is_rational(&self) -> bool {
        self.rational
    }

    pub fn domain(&self) -> RangeInclusive<Real> {
        self.knots[self.degree]..=self.knots[self.control_points.len()]
    }

    pub fn spans(&self) -> impl Iterator<Item = (Real, Real)> + '_ {
        self.knots
            .windows(2)
            .skip(self.degree)
            .take(self.control_points.len() - self.degree)
            .filter_map(|knots| (knots[0] < knots[1]).then_some((knots[0], knots[1])))
    }

    pub fn parameter_at(&self, normalized: Real) -> Result<Real, GeometryError> {
        if !normalized.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "normalized parameter-space NURBS parameter",
            });
        }
        if !(0.0..=1.0).contains(&normalized) {
            return Err(GeometryError::ParameterOutOfDomain {
                parameter: normalized,
                domain_start: 0.0,
                domain_end: 1.0,
            });
        }
        let domain = self.domain();
        let parameter = domain
            .start()
            .mul_add(1.0 - normalized, domain.end() * normalized);
        require_finite([parameter], "parameter-space NURBS parameter")?;
        Ok(parameter)
    }

    pub fn evaluate(&self, parameter: Real) -> Result<Point2, GeometryError> {
        let span = self.checked_span(parameter)?;
        let active = self.active_homogeneous_control_points(span)?;
        let homogeneous = de_boor(&self.knots, self.degree, span, parameter, active)?;
        project_homogeneous(homogeneous)
    }

    /// Evaluates the point and exact first derivative of this rational p-curve.
    pub fn evaluate_with_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point2, [Real; 2]), GeometryError> {
        let span = self.checked_span(parameter)?;
        let active = self.active_homogeneous_control_points(span)?;
        let homogeneous = de_boor(&self.knots, self.degree, span, parameter, active.clone())?;
        let point = project_homogeneous(homogeneous)?;

        let first_control_point = span - self.degree;
        let mut derivative_controls = Vec::with_capacity(self.degree);
        for local_index in 0..self.degree {
            let control_point_index = first_control_point + local_index;
            let knot_start = self.knots[control_point_index + 1];
            let knot_end = self.knots[control_point_index + self.degree + 1];
            let mut derivative = [0.0; 3];
            for coordinate in 0..3 {
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
        let weight = homogeneous[2];
        let weight_derivative = homogeneous_derivative[2];
        let derivative = [
            (-point.x()).mul_add(weight_derivative, homogeneous_derivative[0]) / weight,
            (-point.y()).mul_add(weight_derivative, homogeneous_derivative[1]) / weight,
        ];
        require_finite(derivative, "parameter-space NURBS derivative")?;
        Ok((point, derivative))
    }

    pub fn start_point(&self) -> Result<Point2, GeometryError> {
        self.evaluate(*self.domain().start())
    }

    pub fn end_point(&self) -> Result<Point2, GeometryError> {
        self.evaluate(*self.domain().end())
    }

    /// Reverses direction and negates the knot vector, matching OpenNURBS.
    pub fn reversed(&self) -> Result<Self, GeometryError> {
        let control_points = self.control_points.iter().rev().copied().collect();
        let knots = self.knots.iter().rev().map(|knot| -*knot).collect();
        Self::try_new_rational(self.degree, control_points, knots)
    }

    fn checked_span(&self, parameter: Real) -> Result<usize, GeometryError> {
        require_finite([parameter], "parameter-space NURBS parameter")?;
        let domain = self.domain();
        let domain_start = *domain.start();
        let domain_end = *domain.end();
        if parameter < domain_start || parameter > domain_end {
            return Err(GeometryError::ParameterOutOfDomain {
                parameter,
                domain_start,
                domain_end,
            });
        }
        Ok(find_span_in_knots(
            &self.knots,
            self.degree,
            self.control_points.len(),
            parameter,
        ))
    }

    fn active_homogeneous_control_points(
        &self,
        span: usize,
    ) -> Result<Vec<[Real; 3]>, GeometryError> {
        let first = span - self.degree;
        let active = &self.control_points[first..=span];
        let weight_scale = active
            .iter()
            .map(|control_point| control_point.weight.abs())
            .fold(0.0, Real::max);
        let mut homogeneous = Vec::with_capacity(active.len());
        for control_point in active {
            let weight = control_point.weight / weight_scale;
            let point = control_point.point;
            let value = [point.x() * weight, point.y() * weight, weight];
            require_finite(value, "homogeneous parameter-space NURBS control point")?;
            homogeneous.push(value);
        }
        Ok(homogeneous)
    }
}

fn project_homogeneous(homogeneous: [Real; 3]) -> Result<Point2, GeometryError> {
    let weight = homogeneous[2];
    if !weight.is_finite() || weight == 0.0 {
        return Err(GeometryError::ZeroWeightAtParameter);
    }
    Point2::try_new(homogeneous[0] / weight, homogeneous[1] / weight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tolerance;

    fn point(x: Real, y: Real) -> Point2 {
        Point2::try_new(x, y).unwrap()
    }

    #[test]
    fn parameter_curve_evaluates_lines_and_rational_arcs() {
        let line = NurbsCurve2::try_line(point(-2.0, 3.0), point(4.0, 9.0)).unwrap();
        assert_eq!(line.degree(), 1);
        assert_eq!(line.domain(), 0.0..=1.0);
        assert_eq!(line.evaluate(0.25).unwrap(), point(-0.5, 4.5));

        let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
        let arc = NurbsCurve2::try_new_rational(
            2,
            vec![
                WeightedPoint2::try_new(point(1.0, 0.0), 1.0).unwrap(),
                WeightedPoint2::try_new(point(1.0, 1.0), diagonal_weight).unwrap(),
                WeightedPoint2::try_new(point(0.0, 1.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let middle = arc.evaluate(0.5).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(middle.x(), diagonal_weight));
        assert!(Tolerance::DEFAULT.approx_eq(middle.y(), diagonal_weight));

        let parameter = 0.35;
        let (evaluated, derivative) = arc.evaluate_with_derivative(parameter).unwrap();
        assert_eq!(evaluated, arc.evaluate(parameter).unwrap());
        let step = 1.0e-6;
        let before = arc.evaluate(parameter - step).unwrap();
        let after = arc.evaluate(parameter + step).unwrap();
        assert!((derivative[0] - (after.x() - before.x()) / (2.0 * step)).abs() < 1.0e-9);
        assert!((derivative[1] - (after.y() - before.y()) / (2.0 * step)).abs() < 1.0e-9);
    }

    #[test]
    fn parameter_curve_reversal_preserves_locus_and_swaps_ends() {
        let curve = NurbsCurve2::try_line(point(2.0, 3.0), point(5.0, 7.0)).unwrap();
        let reversed = curve.reversed().unwrap();
        assert_eq!(reversed.domain(), -1.0..=0.0);
        assert_eq!(reversed.start_point().unwrap(), curve.end_point().unwrap());
        assert_eq!(reversed.end_point().unwrap(), curve.start_point().unwrap());
        for index in 0..=8 {
            let normalized = index as Real / 8.0;
            assert_eq!(
                reversed
                    .evaluate(reversed.parameter_at(normalized).unwrap())
                    .unwrap(),
                curve
                    .evaluate(curve.parameter_at(1.0 - normalized).unwrap())
                    .unwrap()
            );
        }
    }

    #[test]
    fn parameter_curve_rejects_invalid_structure_and_weights() {
        assert!(NurbsCurve2::try_line(point(1.0, 2.0), point(1.0, 2.0)).is_err());
        assert!(NurbsCurve2::try_new(0, vec![point(0.0, 0.0)], vec![0.0, 0.0]).is_err());
        assert!(
            NurbsCurve2::try_new_rational(
                1,
                vec![
                    WeightedPoint2::try_new(point(0.0, 0.0), 1.0).unwrap(),
                    WeightedPoint2 {
                        point: point(1.0, 0.0),
                        weight: Real::NAN,
                    },
                ],
                vec![0.0, 0.0, 1.0, 1.0],
            )
            .is_err()
        );
    }

    #[test]
    fn parameter_curve_supports_negative_projective_weights() {
        assert!(WeightedPoint2::try_new(point(0.0, 0.0), 0.0).is_err());
        let curve = NurbsCurve2::try_new_rational(
            2,
            vec![
                WeightedPoint2::try_new(point(0.0, 0.0), 1.0).unwrap(),
                WeightedPoint2::try_new(point(2.0, 3.0), -0.2).unwrap(),
                WeightedPoint2::try_new(point(5.0, 0.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let middle = curve.evaluate(0.5).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(middle.x(), 2.625));
        assert!(Tolerance::DEFAULT.approx_eq(middle.y(), -0.75));
    }
}
