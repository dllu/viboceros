use std::ops::RangeInclusive;

use crate::{AffineTransform3, BoundingBox3, GeometryError, Point3, Real, Vector3, require_finite};

/// A Euclidean control point paired with a strictly positive rational weight.
///
/// Positive weights make every evaluated point a convex combination of its
/// active control points and guarantee a nonzero rational denominator in exact
/// arithmetic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedPoint3 {
    point: Point3,
    weight: Real,
}

impl WeightedPoint3 {
    pub fn try_new(point: Point3, weight: Real) -> Result<Self, GeometryError> {
        if weight.is_finite() && weight > 0.0 {
            Ok(Self { point, weight })
        } else {
            Err(GeometryError::InvalidWeight { index: 0 })
        }
    }

    #[inline]
    pub const fn point(self) -> Point3 {
        self.point
    }

    #[inline]
    pub const fn weight(self) -> Real {
        self.weight
    }
}

/// A finite, non-uniform rational B-spline curve using a standard full knot
/// vector of length `control_point_count + degree + 1`.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve {
    degree: usize,
    control_points: Vec<WeightedPoint3>,
    knots: Vec<Real>,
    rational: bool,
}

impl NurbsCurve {
    /// Constructs a non-rational NURBS curve (all weights are one).
    pub fn try_new(
        degree: usize,
        control_points: Vec<Point3>,
        knots: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        let control_points = control_points
            .into_iter()
            .map(|point| WeightedPoint3 { point, weight: 1.0 })
            .collect();
        Self::try_new_rational(degree, control_points, knots)
    }

    /// Constructs a rational NURBS curve after validating its full knot vector
    /// and all control-point weights.
    pub fn try_new_rational(
        degree: usize,
        control_points: Vec<WeightedPoint3>,
        knots: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        validate_structure(degree, &control_points, &knots)?;
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

    /// Constructs an open, clamped, uniformly spaced non-rational curve.
    pub fn try_clamped_uniform(
        degree: usize,
        control_points: Vec<Point3>,
    ) -> Result<Self, GeometryError> {
        let knots = clamped_uniform_knots(degree, control_points.len())?;
        Self::try_new(degree, control_points, knots)
    }

    #[inline]
    pub const fn degree(&self) -> usize {
        self.degree
    }

    #[inline]
    pub fn control_points(&self) -> &[WeightedPoint3] {
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

    /// Returns every nonempty knot span in the active curve domain.
    pub fn spans(&self) -> impl Iterator<Item = (Real, Real)> + '_ {
        self.knots
            .windows(2)
            .skip(self.degree)
            .take(self.control_points.len() - self.degree)
            .filter_map(|knots| (knots[0] < knots[1]).then_some((knots[0], knots[1])))
    }

    pub fn control_point_bounds(&self) -> BoundingBox3 {
        BoundingBox3::from_points(
            self.control_points
                .iter()
                .map(|control_point| control_point.point),
        )
        .expect("a valid NURBS curve has control points")
    }

    /// Maps a normalized value in `[0, 1]` into the curve domain without
    /// subtracting potentially opposite, very large endpoints.
    pub fn parameter_at(&self, normalized: Real) -> Result<Real, GeometryError> {
        if !normalized.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "normalized NURBS parameter",
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
        let start = *domain.start();
        let end = *domain.end();
        let parameter = start.mul_add(1.0 - normalized, end * normalized);
        require_finite([parameter], "NURBS parameter")?;
        Ok(parameter)
    }

    /// Evaluates the curve with the homogeneous de Boor algorithm.
    pub fn evaluate(&self, parameter: Real) -> Result<Point3, GeometryError> {
        let span = self.checked_span(parameter)?;
        let work = self.active_homogeneous_control_points(span)?;
        let homogeneous = de_boor(&self.knots, self.degree, span, parameter, work)?;
        project_homogeneous(homogeneous)
    }

    /// Evaluates the point and exact first derivative using the derivative
    /// control polygon in homogeneous coordinates and the rational quotient
    /// rule.
    pub fn evaluate_with_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point3, Vector3), GeometryError> {
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
            point,
            Vector3::try_new(derivative[0], derivative[1], derivative[2])?,
        ))
    }

    pub fn derivative_at(&self, parameter: Real) -> Result<Vector3, GeometryError> {
        self.evaluate_with_derivative(parameter)
            .map(|(_, derivative)| derivative)
    }

    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        let control_points = self
            .control_points
            .iter()
            .map(|control_point| {
                Ok(WeightedPoint3 {
                    point: transform.transform_point(control_point.point)?,
                    weight: control_point.weight,
                })
            })
            .collect::<Result<_, GeometryError>>()?;
        Self::try_new_rational(self.degree, control_points, self.knots.clone())
    }

    fn checked_span(&self, parameter: Real) -> Result<usize, GeometryError> {
        require_finite([parameter], "NURBS parameter")?;
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
        Ok(self.find_span(parameter))
    }

    fn active_homogeneous_control_points(
        &self,
        span: usize,
    ) -> Result<Vec<[Real; 4]>, GeometryError> {
        let first_control_point = span - self.degree;
        let active = &self.control_points[first_control_point..=span];
        let weight_scale = active
            .iter()
            .map(|control_point| control_point.weight)
            .fold(0.0, Real::max);
        let mut homogeneous = Vec::with_capacity(active.len());
        for control_point in active {
            let weight = control_point.weight / weight_scale;
            let point = control_point.point;
            let value = [
                point.x() * weight,
                point.y() * weight,
                point.z() * weight,
                weight,
            ];
            require_finite(value, "homogeneous NURBS control point")?;
            homogeneous.push(value);
        }
        Ok(homogeneous)
    }

    fn find_span(&self, parameter: Real) -> usize {
        let last_control_point = self.control_points.len() - 1;
        if parameter >= self.knots[last_control_point + 1] {
            return last_control_point;
        }
        if parameter <= self.knots[self.degree] {
            return self.degree;
        }

        let mut low = self.degree;
        let mut high = last_control_point + 1;
        let mut middle = (low + high) / 2;
        while parameter < self.knots[middle] || parameter >= self.knots[middle + 1] {
            if parameter < self.knots[middle] {
                high = middle;
            } else {
                low = middle;
            }
            middle = (low + high) / 2;
        }
        middle
    }
}

pub(crate) fn de_boor(
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: Real,
    mut work: Vec<[Real; 4]>,
) -> Result<[Real; 4], GeometryError> {
    debug_assert_eq!(work.len(), degree + 1);
    for level in 1..=degree {
        for local_index in (level..=degree).rev() {
            let knot_index = span - degree + local_index;
            let left_knot = knots[knot_index];
            let right_knot = knots[knot_index + degree - level + 1];
            let alpha = interval_fraction(parameter, left_knot, right_knot)?;
            work[local_index] = blend_homogeneous(work[local_index - 1], work[local_index], alpha)?;
        }
    }
    Ok(work[degree])
}

pub(crate) fn project_homogeneous(homogeneous: [Real; 4]) -> Result<Point3, GeometryError> {
    let weight = homogeneous[3];
    if !weight.is_finite() || weight <= 0.0 {
        return Err(GeometryError::ZeroWeightAtParameter);
    }
    Point3::try_new(
        homogeneous[0] / weight,
        homogeneous[1] / weight,
        homogeneous[2] / weight,
    )
}

pub(crate) fn stable_divided_difference(
    right: Real,
    left: Real,
    degree: usize,
    interval_start: Real,
    interval_end: Real,
) -> Result<Real, GeometryError> {
    if right == left {
        return Ok(0.0);
    }

    let direct_numerator = right - left;
    let (numerator, numerator_factor) = if direct_numerator.is_finite() {
        (direct_numerator, 1.0)
    } else {
        (right * 0.5 - left * 0.5, 2.0)
    };
    let direct_denominator = interval_end - interval_start;
    let (denominator, denominator_factor) = if direct_denominator.is_finite() {
        (direct_denominator, 1.0)
    } else {
        (interval_end * 0.5 - interval_start * 0.5, 2.0)
    };
    if !numerator.is_finite() || !denominator.is_finite() || denominator <= 0.0 || numerator == 0.0
    {
        return Err(GeometryError::NonFinite {
            context: "NURBS derivative divided difference",
        });
    }

    let multiplier = degree as Real * numerator_factor / denominator_factor;
    let candidates = [
        (numerator / denominator) * multiplier,
        (numerator * multiplier) / denominator,
    ];
    if let Some(value) = candidates
        .into_iter()
        .find(|value| value.is_finite() && *value != 0.0)
    {
        Ok(value)
    } else if candidates.into_iter().any(|value| value == 0.0) {
        Ok(0.0)
    } else {
        Err(GeometryError::NonFinite {
            context: "NURBS derivative divided difference",
        })
    }
}

fn interval_fraction(
    value: Real,
    interval_start: Real,
    interval_end: Real,
) -> Result<Real, GeometryError> {
    let denominator = interval_end - interval_start;
    let alpha = if denominator.is_finite() && denominator > 0.0 {
        (value - interval_start) / denominator
    } else if denominator.is_infinite() && interval_start < interval_end {
        // Halving preserves the ratio when subtracting opposite, very large
        // finite endpoints would overflow.
        let scaled_start = interval_start * 0.5;
        (value * 0.5 - scaled_start) / (interval_end * 0.5 - scaled_start)
    } else {
        return Err(GeometryError::InvalidKnotVector {
            context: "an active de Boor knot interval is empty",
        });
    };
    if !alpha.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "de Boor blend factor",
        });
    }
    // The validated span brackets `value`; a value just outside this range can
    // only be floating-point roundoff in the ratio calculation.
    Ok(alpha.clamp(0.0, 1.0))
}

fn validate_structure(
    degree: usize,
    control_points: &[WeightedPoint3],
    knots: &[Real],
) -> Result<(), GeometryError> {
    validate_direction(degree, control_points.len(), knots)?;
    for (index, control_point) in control_points.iter().enumerate() {
        if !control_point.weight.is_finite() || control_point.weight <= 0.0 {
            return Err(GeometryError::InvalidWeight { index });
        }
    }

    Ok(())
}

pub(crate) fn validate_direction(
    degree: usize,
    control_point_count: usize,
    knots: &[Real],
) -> Result<(), GeometryError> {
    validate_control_point_count(degree, control_point_count)?;
    let expected_knot_count = control_point_count
        .checked_add(degree)
        .and_then(|count| count.checked_add(1))
        .ok_or(GeometryError::InvalidKnotVector {
            context: "knot count overflowed usize",
        })?;
    if knots.len() != expected_knot_count {
        return Err(GeometryError::InvalidKnotCount {
            expected: expected_knot_count,
            actual: knots.len(),
        });
    }
    if !knots.iter().all(|knot| knot.is_finite()) {
        return Err(GeometryError::InvalidKnotVector {
            context: "knots must be finite",
        });
    }
    if knots.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(GeometryError::InvalidKnotVector {
            context: "knots must be nondecreasing",
        });
    }
    if knots[degree] >= knots[control_point_count] {
        return Err(GeometryError::InvalidKnotVector {
            context: "the active domain must have positive length",
        });
    }

    let maximum_multiplicity = degree + 1;
    let mut run_length = 1;
    for pair in knots.windows(2) {
        if pair[0] == pair[1] {
            run_length += 1;
            if run_length > maximum_multiplicity {
                return Err(GeometryError::InvalidKnotVector {
                    context: "knot multiplicity exceeds degree plus one",
                });
            }
        } else {
            run_length = 1;
        }
    }

    Ok(())
}

pub(crate) fn clamped_uniform_knots(
    degree: usize,
    control_point_count: usize,
) -> Result<Vec<Real>, GeometryError> {
    validate_control_point_count(degree, control_point_count)?;
    let knot_count = control_point_count
        .checked_add(degree)
        .and_then(|count| count.checked_add(1))
        .ok_or(GeometryError::InvalidKnotVector {
            context: "knot count overflowed usize",
        })?;
    let span_count = control_point_count - degree;
    Ok((0..knot_count)
        .map(|index| {
            if index <= degree {
                0.0
            } else if index >= control_point_count {
                1.0
            } else {
                (index - degree) as Real / span_count as Real
            }
        })
        .collect())
}

fn validate_control_point_count(degree: usize, count: usize) -> Result<(), GeometryError> {
    if degree == 0 {
        return Err(GeometryError::InvalidDegree);
    }
    let required = degree.checked_add(1).ok_or(GeometryError::InvalidDegree)?;
    if count < required {
        return Err(GeometryError::InsufficientControlPoints {
            degree,
            required,
            actual: count,
        });
    }
    Ok(())
}

fn blend_homogeneous(
    left: [Real; 4],
    right: [Real; 4],
    alpha: Real,
) -> Result<[Real; 4], GeometryError> {
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return Err(GeometryError::InvalidKnotVector {
            context: "de Boor blend factor is outside zero to one",
        });
    }
    let complement = 1.0 - alpha;
    let result = std::array::from_fn(|index| left[index].mul_add(complement, right[index] * alpha));
    require_finite(result, "homogeneous NURBS evaluation")?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tolerance;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn assert_point_near(actual: Point3, expected: Point3) {
        assert!(actual.is_near(
            expected,
            Tolerance::try_new(1.0e-12, 1.0e-12, 1.0e-12).unwrap()
        ));
    }

    #[test]
    fn degree_one_curve_interpolates_linearly() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(2.0, 4.0)],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(curve.evaluate(0.0).unwrap(), point(0.0, 0.0));
        assert_eq!(curve.evaluate(1.0).unwrap(), point(2.0, 4.0));
        assert_eq!(curve.evaluate(0.25).unwrap(), point(0.5, 1.0));
        assert_eq!(
            curve.derivative_at(0.25).unwrap(),
            Vector3::try_new(2.0, 4.0, 0.0).unwrap()
        );
    }

    #[test]
    fn quadratic_bezier_matches_bernstein_result() {
        let curve = NurbsCurve::try_new(
            2,
            vec![point(0.0, 0.0), point(1.0, 2.0), point(2.0, 0.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert_point_near(curve.evaluate(0.5).unwrap(), point(1.0, 1.0));
        assert_eq!(
            curve.derivative_at(0.5).unwrap(),
            Vector3::try_new(2.0, 0.0, 0.0).unwrap()
        );
    }

    #[test]
    fn rational_quadratic_represents_exact_quarter_circle() {
        let middle_weight = 0.5_f64.sqrt();
        let controls = vec![
            WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 1.0), middle_weight).unwrap(),
            WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
        ];
        let curve =
            NurbsCurve::try_new_rational(2, controls, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
        let coordinate = 0.5_f64.sqrt();
        let midpoint = curve.evaluate(0.5).unwrap();
        assert_point_near(midpoint, point(coordinate, coordinate));
        assert!(Tolerance::DEFAULT.approx_eq(midpoint.x().hypot(midpoint.y()), 1.0));
        let tangent = curve.derivative_at(0.5).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(tangent.x() + tangent.y(), 0.0));
        let radius = Vector3::try_new(midpoint.x(), midpoint.y(), 0.0).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(radius.dot(tangent).unwrap(), 0.0));
    }

    #[test]
    fn clamped_uniform_curve_has_expected_knots_and_endpoints() {
        let controls = vec![
            point(0.0, 0.0),
            point(1.0, 2.0),
            point(2.0, 2.0),
            point(3.0, 0.0),
            point(4.0, 1.0),
        ];
        let curve = NurbsCurve::try_clamped_uniform(3, controls.clone()).unwrap();
        assert_eq!(
            curve.knots(),
            &[0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]
        );
        assert_eq!(curve.evaluate(0.0).unwrap(), controls[0]);
        assert_eq!(curve.evaluate(1.0).unwrap(), controls[4]);
    }

    #[test]
    fn rejects_structural_errors_and_out_of_domain_parameters() {
        let controls = vec![point(0.0, 0.0), point(1.0, 1.0), point(2.0, 0.0)];
        assert!(NurbsCurve::try_new(0, controls.clone(), vec![]).is_err());
        assert!(NurbsCurve::try_new(2, controls.clone(), vec![0.0; 5]).is_err());
        assert!(
            NurbsCurve::try_new(2, controls.clone(), vec![0.0, 0.0, 0.5, 0.4, 1.0, 1.0]).is_err()
        );

        let curve = NurbsCurve::try_clamped_uniform(2, controls).unwrap();
        assert!(matches!(
            curve.evaluate(-0.1),
            Err(GeometryError::ParameterOutOfDomain { .. })
        ));
        assert!(curve.evaluate(Real::NAN).is_err());
    }

    #[test]
    fn uniformly_scaling_weights_does_not_change_curve() {
        let points = [point(0.0, 0.0), point(1.0, 2.0), point(2.0, 0.0)];
        let make_curve = |scale: Real| {
            NurbsCurve::try_new_rational(
                2,
                points
                    .into_iter()
                    .zip([1.0, 0.25, 2.0])
                    .map(|(point, weight)| WeightedPoint3::try_new(point, weight * scale).unwrap())
                    .collect(),
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            )
            .unwrap()
        };
        let ordinary = make_curve(1.0);
        let huge = make_curve(1.0e200);
        assert_point_near(
            ordinary.evaluate(0.37).unwrap(),
            huge.evaluate(0.37).unwrap(),
        );
    }

    #[test]
    fn affine_transform_preserves_knots_weights_and_evaluation() {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(0.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 2.0), 0.5).unwrap(),
                WeightedPoint3::try_new(point(3.0, 0.0), 2.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let transform = AffineTransform3::try_new(
            [[2.0, -1.0, 0.0], [0.5, 3.0, 0.0], [0.0, 0.0, 1.0]],
            Vector3::try_new(4.0, -2.0, 7.0).unwrap(),
        )
        .unwrap();
        let transformed = curve.transformed(transform).unwrap();

        assert_eq!(transformed.knots(), curve.knots());
        assert_eq!(
            transformed
                .control_points()
                .iter()
                .map(|control| control.weight())
                .collect::<Vec<_>>(),
            vec![1.0, 0.5, 2.0]
        );
        assert_point_near(
            transformed.evaluate(0.37).unwrap(),
            transform
                .transform_point(curve.evaluate(0.37).unwrap())
                .unwrap(),
        );
    }

    #[test]
    fn evaluates_across_a_domain_whose_difference_overflows() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(10.0, 0.0)],
            vec![-Real::MAX, -Real::MAX, Real::MAX, Real::MAX],
        )
        .unwrap();
        assert_eq!(curve.parameter_at(0.5).unwrap(), 0.0);
        assert_point_near(curve.evaluate(0.0).unwrap(), point(5.0, 0.0));
        let derivative = curve.derivative_at(0.0).unwrap();
        assert!(derivative.x().is_finite());
        assert!(derivative.x() > 0.0);
    }

    #[test]
    fn fully_multiple_knot_selects_right_hand_piece() {
        let curve = NurbsCurve::try_new(
            1,
            vec![
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(10.0, 0.0),
                point(11.0, 0.0),
            ],
            vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(curve.evaluate(0.5).unwrap(), point(10.0, 0.0));
        assert!(curve.evaluate(0.5_f64.next_down()).unwrap().x() < 1.0);
        assert_eq!(curve.derivative_at(0.5).unwrap().x(), 2.0);
    }
}
