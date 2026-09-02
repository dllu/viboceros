use std::ops::RangeInclusive;

use crate::{
    AffineTransform3, BoundingBox3, GeometryError, Point3, Polyline3, Real, Tolerance, Vector3,
    integration::integrate_adaptive, require_finite,
};

// OpenNURBS uses these fixed coordinate tolerances for Curve::IsClosed.
pub(crate) const CURVE_COINCIDENCE_ABSOLUTE: Real = 2.328_306_436_538_696_3e-10;
const CURVE_COINCIDENCE_RELATIVE: Real = 2.273_736_754_432_320_6e-13;
const OPENNURBS_SQRT_EPSILON: Real = 1.490_116_119_385e-8;

/// Topology requested for a Rhino `Curve`-style control-point curve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlPointCurveClosure {
    /// Leaves the natural endpoints independent.
    Open,
    /// Repeats controls and uniform knots to form a smooth periodic seam.
    Smooth,
    /// Repeats only the first control to form a non-periodic kinked seam.
    Sharp,
}

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

    /// Constructs a Rhino `Curve`-style open control-point curve.
    ///
    /// The effective degree is lowered to `control_point_count - 1` when the
    /// requested degree is too high. Knots are clamped and uniform, and the
    /// parameter domain is scaled to the control polygon's total length.
    pub fn try_control_point_curve(
        requested_degree: usize,
        control_points: Vec<Point3>,
    ) -> Result<Self, GeometryError> {
        Self::try_control_point_curve_with_closure(
            requested_degree,
            control_points,
            ControlPointCurveClosure::Open,
        )
    }

    /// Constructs a Rhino `Curve`-style control-point curve with the requested
    /// seam topology.
    pub fn try_control_point_curve_with_closure(
        requested_degree: usize,
        mut control_points: Vec<Point3>,
        closure: ControlPointCurveClosure,
    ) -> Result<Self, GeometryError> {
        if requested_degree == 0 {
            return Err(GeometryError::InvalidDegree);
        }

        if closure != ControlPointCurveClosure::Open
            && control_points.len() > 1
            && control_points.first() == control_points.last()
        {
            control_points.pop();
        }

        match closure {
            ControlPointCurveClosure::Open => {
                if control_points.len() < 2 {
                    return Err(GeometryError::InsufficientControlPoints {
                        degree: 1,
                        required: 2,
                        actual: control_points.len(),
                    });
                }
                let degree = requested_degree.min(control_points.len() - 1);
                clamped_control_point_curve(degree, control_points)
            }
            ControlPointCurveClosure::Smooth | ControlPointCurveClosure::Sharp => {
                if control_points.len() < 3 {
                    return Err(GeometryError::InsufficientClosedControlPoints {
                        actual: control_points.len(),
                    });
                }
                if closure == ControlPointCurveClosure::Smooth {
                    let degree = requested_degree.min(control_points.len());
                    periodic_control_point_curve(degree, control_points)
                } else {
                    control_points.push(control_points[0]);
                    let degree = requested_degree.min(control_points.len() - 1);
                    clamped_control_point_curve(degree, control_points)
                }
            }
        }
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

    /// Affinely maps the full knot vector onto a new active parameter domain.
    ///
    /// Control points and weights are unchanged, so the geometric image and
    /// normalized parameter direction are preserved. Knots outside the active
    /// domain, as used by periodic curves, are extrapolated by the same map.
    pub fn try_reparameterized(&self, domain: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        let target_start = *domain.start();
        let target_end = *domain.end();
        if !target_start.is_finite() || !target_end.is_finite() {
            return Err(GeometryError::InvalidKnotVector {
                context: "the reparameterized domain must be finite",
            });
        }
        if target_start >= target_end {
            return Err(GeometryError::InvalidKnotVector {
                context: "the reparameterized domain must have positive length",
            });
        }

        let source = self.domain();
        let source_start = *source.start();
        let source_end = *source.end();
        let knots = self
            .knots
            .iter()
            .map(|knot| {
                if *knot == source_start {
                    Ok(target_start)
                } else if *knot == source_end {
                    Ok(target_end)
                } else {
                    reparameterize_value(*knot, source_start, source_end, target_start, target_end)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new_rational(self.degree, self.control_points.clone(), knots)
    }

    /// Returns every nonempty knot span in the active curve domain.
    pub fn spans(&self) -> impl Iterator<Item = (Real, Real)> + '_ {
        self.knots
            .windows(2)
            .skip(self.degree)
            .take(self.control_points.len() - self.degree)
            .filter_map(|knots| (knots[0] < knots[1]).then_some((knots[0], knots[1])))
    }

    /// Returns the OpenNURBS-compatible topological closed state.
    ///
    /// NURBS curves need at least four controls, coincident natural endpoints,
    /// and interior samples distinct from both ends. Endpoint coincidence uses
    /// OpenNURBS' fixed zero/relative tolerances rather than document tolerance.
    pub fn is_closed(&self) -> Result<bool, GeometryError> {
        if self.control_points.len() < 4 {
            return Ok(false);
        }
        if self.is_periodic() {
            return Ok(true);
        }
        let start = self.evaluate(*self.domain().start())?;
        let end = self.evaluate(*self.domain().end())?;
        if !curve_points_coincident(start, end) {
            return Ok(false);
        }
        let first_interior = self.evaluate(self.parameter_at(1.0 / 3.0)?)?;
        let second_interior = self.evaluate(self.parameter_at(2.0 / 3.0)?)?;
        Ok(!curve_points_coincident(start, first_interior)
            && !curve_points_coincident(start, second_interior)
            && !curve_points_coincident(end, first_interior)
            && !curve_points_coincident(end, second_interior))
    }

    /// Returns whether knots and repeated end controls form an OpenNURBS-style
    /// periodic curve. By convention, degree-one curves are never periodic.
    pub fn is_periodic(&self) -> bool {
        let order = self.degree + 1;
        let control_count = self.control_points.len();
        let short_knots = &self.knots[1..self.knots.len() - 1];
        if !knot_vector_is_periodic(order, control_count, short_knots) {
            return false;
        }
        (0..self.degree).all(|index| {
            let left = self.control_points[index].point;
            let right = self.control_points[control_count - self.degree + index].point;
            curve_points_coincident(left, right)
        })
    }

    /// Returns control-point locations in Rhino `ExtractPt` grip order.
    /// Periodic curves omit their repeated tail controls. Non-periodic closed
    /// curves omit an exactly duplicated final seam control, while
    /// near-coincident controls remain distinct.
    pub fn extract_point_locations(&self) -> Result<Vec<Point3>, GeometryError> {
        let mut points = self
            .control_points
            .iter()
            .map(|control_point| control_point.point)
            .collect::<Vec<_>>();
        if self.is_periodic() {
            points.truncate(points.len() - self.degree);
        } else if self.is_closed()? && points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        Ok(points)
    }

    /// Fits Rhino's extracted degree-one control polygon through this curve's
    /// Euclidean control locations.
    ///
    /// Periodic curves use the domain-aligned Greville window returned by
    /// `Curve.ControlPolygon`, retaining one repeated endpoint to close the
    /// polyline. Unlike `ExtractPt`, this can therefore rotate the first
    /// output point away from raw control index zero.
    pub fn control_polygon(&self, tolerance: Tolerance) -> Result<Polyline3, GeometryError> {
        let (start, end) = control_polygon_range(
            self.degree,
            self.control_points.len(),
            &self.knots,
            self.is_periodic(),
        );
        let mut points = self.control_points[start..end]
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>();
        if self.is_periodic() {
            let first = points[0];
            *points
                .last_mut()
                .expect("a periodic control window is nonempty") = first;
        }
        Polyline3::try_new(points, tolerance)
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

    /// Computes arc length span by span with adaptive Gauss-Kronrod
    /// integration of the exact first derivative.
    pub fn length(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        let spans = self.spans().collect::<Vec<_>>();
        let absolute_per_span = tolerance.absolute() / spans.len() as Real;
        if absolute_per_span <= 0.0 {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (start, end) in spans {
            let length = integrate_adaptive(
                start,
                end,
                absolute_per_span,
                tolerance.relative(),
                |parameter| self.derivative_at(parameter)?.length(),
            )?;
            let next = sum + length;
            if sum.abs() >= length.abs() {
                correction += (sum - next) + length;
            } else {
                correction += (length - next) + sum;
            }
            sum = next;
        }
        let length = sum + correction;
        require_finite([length], "NURBS curve length")?;
        Ok(length)
    }

    /// Reverses direction by reversing the controls and negating the full
    /// knot vector. A domain `[a, b]` therefore becomes `[-b, -a]`, matching
    /// the OpenNURBS/Rhino convention.
    pub fn reversed(&self) -> Result<Self, GeometryError> {
        let control_points = self.control_points.iter().rev().copied().collect();
        let knots = self.knots.iter().rev().map(|knot| -*knot).collect();
        Self::try_new_rational(self.degree, control_points, knots)
    }

    /// Returns the exact full-knot-vector multiplicity of `parameter`.
    ///
    /// Knot equality is intentionally exact. Near knots remain distinct so a
    /// refinement never changes the caller's requested parameter value.
    pub fn knot_multiplicity(&self, parameter: Real) -> Result<usize, GeometryError> {
        self.validate_parameter(parameter)?;
        Ok(self.knots.iter().filter(|knot| **knot == parameter).count())
    }

    /// Inserts `parameter` until its full-knot-vector multiplicity is at least
    /// `target_multiplicity`, without changing the curve's parameterization or
    /// geometric image.
    ///
    /// A multiplicity of `degree + 1` is supported so callers can represent an
    /// existing discontinuity explicitly. Asking for a multiplicity the curve
    /// already has is a no-op. Refining either active-domain endpoint performs
    /// the equivalent shape-preserving full clamp, because a non-clamped
    /// endpoint is not an ordinary interior knot.
    pub fn try_insert_knot(
        &self,
        parameter: Real,
        target_multiplicity: usize,
    ) -> Result<Self, GeometryError> {
        self.validate_parameter(parameter)?;
        let maximum = self
            .degree
            .checked_add(1)
            .ok_or(GeometryError::InvalidDegree)?;
        if target_multiplicity == 0 || target_multiplicity > maximum {
            return Err(GeometryError::InvalidKnotMultiplicity {
                actual: target_multiplicity,
                maximum,
            });
        }

        let current_multiplicity = self.knot_multiplicity_unchecked(parameter);
        if current_multiplicity >= target_multiplicity {
            return Ok(self.clone());
        }
        let domain = self.domain();
        if parameter == *domain.start() {
            return self.clamped_at_start(parameter);
        }
        if parameter == *domain.end() {
            return self.clamped_at_end(parameter);
        }

        let mut refined = self.clone();
        while refined.knot_multiplicity_unchecked(parameter) < target_multiplicity {
            refined = refined.insert_knot_once(parameter)?;
        }
        Ok(refined)
    }

    /// Splits at a parameter strictly inside the active domain.
    ///
    /// Both results retain the source parameter values and are clamped at
    /// their active ends. At an existing `degree + 1` knot, the independent
    /// left- and right-hand controls remain independent.
    pub fn try_split(&self, parameter: Real) -> Result<(Self, Self), GeometryError> {
        require_finite([parameter], "NURBS curve split parameter")?;
        let domain = self.domain();
        if parameter <= *domain.start() || parameter >= *domain.end() {
            return Err(GeometryError::InvalidCurveSplitParameter);
        }

        let multiplicity = self.knot_multiplicity_unchecked(parameter);
        let refined = if multiplicity < self.degree {
            self.try_insert_knot(parameter, self.degree)?
        } else {
            self.clone()
        };
        let multiplicity = refined.knot_multiplicity_unchecked(parameter);
        let first_knot = refined.knots.partition_point(|knot| *knot < parameter);
        let after_knots = refined.knots.partition_point(|knot| *knot <= parameter);

        let (left_controls, left_knots, right_controls, right_knots) =
            if multiplicity == self.degree + 1 {
                (
                    refined.control_points[..first_knot].to_vec(),
                    refined.knots[..after_knots].to_vec(),
                    refined.control_points[first_knot..].to_vec(),
                    refined.knots[first_knot..].to_vec(),
                )
            } else {
                debug_assert_eq!(multiplicity, self.degree);
                let shared_control = after_knots - self.degree - 1;
                let mut left_knots = refined.knots[..after_knots].to_vec();
                left_knots.push(parameter);
                let mut right_knots =
                    Vec::with_capacity(refined.knots.len() - (shared_control + 1) + 1);
                right_knots.push(parameter);
                right_knots.extend_from_slice(&refined.knots[shared_control + 1..]);
                (
                    refined.control_points[..=shared_control].to_vec(),
                    left_knots,
                    refined.control_points[shared_control..].to_vec(),
                    right_knots,
                )
            };

        let left = Self::try_new_rational(self.degree, left_controls, left_knots)?
            .clamped_to_active_domain()?;
        let right = Self::try_new_rational(self.degree, right_controls, right_knots)?
            .clamped_to_active_domain()?;
        Ok((left, right))
    }

    /// Extracts an exact subcurve while retaining the source parameter values.
    /// Partial trims are clamped at both active ends. Trimming to the exact
    /// existing domain is a no-op, preserving periodic form.
    pub fn try_trimmed(&self, interval: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        let start = *interval.start();
        let end = *interval.end();
        if !start.is_finite() || !end.is_finite() || start >= end {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        let domain = self.domain();
        if start < *domain.start() || end > *domain.end() {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        if start == *domain.start() && end == *domain.end() {
            return Ok(self.clone());
        }

        let after_start = if start == *domain.start() {
            self.clone()
        } else {
            self.try_split(start)?.1
        };
        if end == *domain.end() {
            Ok(after_start)
        } else {
            Ok(after_start.try_split(end)?.0)
        }
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
        self.validate_parameter(parameter)?;
        Ok(self.find_span(parameter))
    }

    fn validate_parameter(&self, parameter: Real) -> Result<(), GeometryError> {
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
        Ok(())
    }

    fn knot_multiplicity_unchecked(&self, parameter: Real) -> usize {
        self.knots.iter().filter(|knot| **knot == parameter).count()
    }

    fn insert_knot_once(&self, parameter: Real) -> Result<Self, GeometryError> {
        let control_count = self.control_points.len();
        let last_control = control_count - 1;
        debug_assert!(parameter > self.knots[self.degree]);
        debug_assert!(parameter < self.knots[control_count]);
        let span = self.knots.partition_point(|knot| *knot <= parameter) - 1;
        let multiplicity = self.knot_multiplicity_unchecked(parameter);
        debug_assert!(multiplicity <= self.degree);
        debug_assert!(span >= self.degree && span <= last_control);

        let first_unchanged = span - self.degree;
        let first_shifted = span - multiplicity + 1;
        let mut controls = Vec::with_capacity(control_count + 1);
        for new_index in 0..=control_count {
            let control = if new_index <= first_unchanged {
                self.control_points[new_index]
            } else if new_index < first_shifted {
                let denominator_start = self.knots[new_index];
                let denominator_end = self.knots[new_index + self.degree];
                let alpha = interval_fraction(parameter, denominator_start, denominator_end)?;
                blend_weighted_control_points(
                    self.control_points[new_index - 1],
                    self.control_points[new_index],
                    alpha,
                )?
            } else {
                self.control_points[new_index - 1]
            };
            controls.push(control);
        }

        let mut knots = Vec::with_capacity(self.knots.len() + 1);
        knots.extend_from_slice(&self.knots[..=span]);
        knots.push(parameter);
        knots.extend_from_slice(&self.knots[span + 1..]);
        Self::try_new_rational(self.degree, controls, knots)
    }

    fn clamped_to_active_domain(&self) -> Result<Self, GeometryError> {
        let start = *self.domain().start();
        let end = *self.domain().end();
        self.clamped_at_start(start)?.clamped_at_end(end)
    }

    fn clamped_at_start(&self, start: Real) -> Result<Self, GeometryError> {
        if self.knots[..=self.degree].iter().all(|knot| *knot == start) {
            return Ok(self.clone());
        }

        let span = self.find_span(start);
        let (_, right_controls) = self.de_boor_side_controls(span, start)?;
        let mut controls = Vec::with_capacity(self.control_points.len() - (span - self.degree));
        controls.extend(right_controls);
        controls.extend_from_slice(&self.control_points[span + 1..]);
        let mut knots = Vec::with_capacity(controls.len() + self.degree + 1);
        knots.resize(self.degree + 1, start);
        knots.extend_from_slice(&self.knots[span + 1..]);
        Self::try_new_rational(self.degree, controls, knots)
    }

    fn clamped_at_end(&self, end: Real) -> Result<Self, GeometryError> {
        if self.knots[self.knots.len() - self.degree - 1..]
            .iter()
            .all(|knot| *knot == end)
        {
            return Ok(self.clone());
        }

        let span = self.find_span(end);
        let (left_controls, _) = self.de_boor_side_controls(span, end)?;
        let first_active_control = span - self.degree;
        let mut controls = Vec::with_capacity(span + 1);
        controls.extend_from_slice(&self.control_points[..first_active_control]);
        controls.extend(left_controls);
        let mut knots = Vec::with_capacity(controls.len() + self.degree + 1);
        knots.extend_from_slice(&self.knots[..=span]);
        knots.resize(knots.len() + self.degree + 1, end);
        Self::try_new_rational(self.degree, controls, knots)
    }

    fn de_boor_side_controls(
        &self,
        span: usize,
        parameter: Real,
    ) -> Result<(Vec<WeightedPoint3>, Vec<WeightedPoint3>), GeometryError> {
        let mut work = self.control_points[span - self.degree..=span].to_vec();
        let mut left = Vec::with_capacity(self.degree + 1);
        let mut right = Vec::with_capacity(self.degree + 1);
        left.push(work[0]);
        right.push(work[self.degree]);
        for level in 1..=self.degree {
            for local_index in (level..=self.degree).rev() {
                let knot_index = span - self.degree + local_index;
                let alpha = interval_fraction(
                    parameter,
                    self.knots[knot_index],
                    self.knots[knot_index + self.degree - level + 1],
                )?;
                work[local_index] =
                    blend_weighted_control_points(work[local_index - 1], work[local_index], alpha)?;
            }
            left.push(work[level]);
            right.push(work[self.degree]);
        }
        right.reverse();
        Ok((left, right))
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
        find_span_in_knots(
            &self.knots,
            self.degree,
            self.control_points.len(),
            parameter,
        )
    }
}

fn control_polygon_length(control_points: &[Point3]) -> Result<Real, GeometryError> {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for pair in control_points.windows(2) {
        let length = pair[0].distance_to(pair[1])?;
        let next = sum + length;
        if sum.abs() >= length.abs() {
            correction += (sum - next) + length;
        } else {
            correction += (length - next) + sum;
        }
        sum = next;
    }
    let length = sum + correction;
    require_finite([length], "control polygon length")?;
    Ok(length)
}

fn reparameterize_value(
    value: Real,
    source_start: Real,
    source_end: Real,
    target_start: Real,
    target_end: Real,
) -> Result<Real, GeometryError> {
    let source_scale = value.abs().max(source_start.abs()).max(source_end.abs());
    debug_assert!(source_scale > 0.0);
    let scaled_source_start = source_start / source_scale;
    let source_span = source_end / source_scale - scaled_source_start;
    if !source_span.is_finite() || source_span <= 0.0 {
        return Err(GeometryError::InvalidKnotVector {
            context: "the source domain cannot be stably reparameterized",
        });
    }
    let normalized = (value / source_scale - scaled_source_start) / source_span;
    if !normalized.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "NURBS reparameterized knot",
        });
    }

    let target_scale = target_start.abs().max(target_end.abs());
    debug_assert!(target_scale > 0.0);
    let scaled_target_start = target_start / target_scale;
    let scaled_target_span = target_end / target_scale - scaled_target_start;
    let mapped = normalized.mul_add(scaled_target_span, scaled_target_start) * target_scale;
    require_finite([mapped], "NURBS reparameterized knot")?;
    Ok(mapped)
}

fn clamped_control_point_curve(
    degree: usize,
    control_points: Vec<Point3>,
) -> Result<NurbsCurve, GeometryError> {
    let domain_end = control_polygon_length(&control_points)?;
    if domain_end <= 0.0 {
        return Err(GeometryError::Degenerate {
            context: "control-point curve",
        });
    }
    let knots = clamped_uniform_knots(degree, control_points.len())?
        .into_iter()
        .map(|knot| knot * domain_end)
        .collect();
    NurbsCurve::try_new(degree, control_points, knots)
}

fn periodic_control_point_curve(
    degree: usize,
    unique_control_points: Vec<Point3>,
) -> Result<NurbsCurve, GeometryError> {
    let unique_count = unique_control_points.len();
    let control_count =
        unique_count
            .checked_add(degree)
            .ok_or(GeometryError::InvalidKnotVector {
                context: "periodic control-point count overflowed usize",
            })?;
    let lead_count = (degree - 1) / 2;
    let tail_count = degree - lead_count;
    let mut control_points = Vec::with_capacity(control_count);
    control_points.extend_from_slice(&unique_control_points[unique_count - lead_count..]);
    control_points.extend_from_slice(&unique_control_points);
    control_points.extend_from_slice(&unique_control_points[..tail_count]);

    let domain_end = control_polygon_length(&control_points)?;
    if domain_end <= 0.0 {
        return Err(GeometryError::Degenerate {
            context: "control-point curve",
        });
    }
    let step = domain_end / unique_count as Real;
    require_finite([step], "periodic control-point curve knot step")?;
    let knot_count = control_count
        .checked_add(degree)
        .and_then(|count| count.checked_add(1))
        .ok_or(GeometryError::InvalidKnotVector {
            context: "periodic control-point curve knot count overflowed usize",
        })?;
    let knots = (0..knot_count)
        .map(|index| (index as Real - degree as Real) * step)
        .collect::<Vec<_>>();
    require_finite(knots.iter().copied(), "periodic control-point curve knots")?;
    NurbsCurve::try_new(degree, control_points, knots)
}

pub(crate) fn bspline_basis_values(
    knots: &[Real],
    degree: usize,
    control_count: usize,
    parameter: Real,
) -> Result<Vec<Real>, GeometryError> {
    debug_assert_eq!(knots.len(), control_count + degree + 1);
    debug_assert!(degree >= 1 && control_count > degree);
    let domain_start = knots[degree];
    let domain_end = knots[control_count];
    require_finite([parameter], "B-spline basis parameter")?;
    if parameter < domain_start || parameter > domain_end {
        return Err(GeometryError::ParameterOutOfDomain {
            parameter,
            domain_start,
            domain_end,
        });
    }
    let span = find_span_in_knots(knots, degree, control_count, parameter);
    let mut local = vec![0.0; degree + 1];
    local[0] = 1.0;
    for column in 1..=degree {
        let mut saved = 0.0;
        for row in 0..column {
            let left_knot = knots[span + 1 - column + row];
            let right_knot = knots[span + row + 1];
            let left_fraction = interval_fraction(parameter, left_knot, right_knot)?;
            let value = local[row];
            local[row] = (1.0 - left_fraction).mul_add(value, saved);
            saved = left_fraction * value;
        }
        local[column] = saved;
    }
    require_finite(local.iter().copied(), "B-spline basis")?;

    let mut values = vec![0.0; control_count];
    values[span - degree..=span].copy_from_slice(&local);
    Ok(values)
}

pub(crate) fn find_span_in_knots(
    knots: &[Real],
    degree: usize,
    control_count: usize,
    parameter: Real,
) -> usize {
    let last_control_point = control_count - 1;
    if parameter >= knots[control_count] {
        // Select the nonempty span immediately to the left. This also handles
        // valid full vectors whose equal end knots straddle the active-domain
        // index instead of occupying only the exterior tail.
        return (knots.partition_point(|knot| *knot < knots[control_count]) - 1)
            .clamp(degree, last_control_point);
    }
    if parameter <= knots[degree] {
        // Select the nonempty span immediately to the right. A clamped vector
        // still returns `degree`; a refined non-clamped start can have more
        // equal knots after that index.
        return (knots.partition_point(|knot| *knot <= knots[degree]) - 1)
            .clamp(degree, last_control_point);
    }

    let mut low = degree;
    let mut high = control_count;
    let mut middle = (low + high) / 2;
    while parameter < knots[middle] || parameter >= knots[middle + 1] {
        if parameter < knots[middle] {
            high = middle;
        } else {
            low = middle;
        }
        middle = (low + high) / 2;
    }
    middle
}

pub(crate) fn curve_points_coincident(left: Point3, right: Point3) -> bool {
    left.to_array()
        .into_iter()
        .zip(right.to_array())
        .all(|(left, right)| {
            let difference = (left - right).abs();
            difference <= CURVE_COINCIDENCE_ABSOLUTE
                || difference <= (left.abs() + right.abs()) * CURVE_COINCIDENCE_RELATIVE
        })
}

/// Returns the half-open raw-control window used by OpenNURBS control
/// polygons. Periodic directions keep one closing control and slide the
/// window until its endpoint Greville abscissae bracket the active domain.
pub(crate) fn control_polygon_range(
    degree: usize,
    control_count: usize,
    knots: &[Real],
    periodic: bool,
) -> (usize, usize) {
    debug_assert!(degree >= 1);
    debug_assert_eq!(knots.len(), control_count + degree + 1);
    if !periodic {
        return (0, control_count);
    }

    let greville = |control: usize| {
        let values = &knots[control + 1..=control + degree];
        let scale = values.iter().map(|value| value.abs()).fold(0.0, Real::max);
        if scale == 0.0 {
            0.0
        } else {
            (values.iter().map(|value| value / scale).sum::<Real>() / degree as Real)
                .clamp(-1.0, 1.0)
                * scale
        }
    };
    let domain_start = knots[degree];
    let domain_end = knots[control_count];
    let mut start = 0;
    let mut end = control_count - (degree - 1);
    while end < control_count && greville(start) < domain_start && greville(end - 1) <= domain_end {
        start += 1;
        end += 1;
    }
    (start, end)
}

pub(crate) fn knot_vector_is_periodic(order: usize, control_count: usize, knots: &[Real]) -> bool {
    if order < 2 || control_count < order || knots.len() != order + control_count - 2 {
        return false;
    }
    if order == 2 {
        return false;
    }
    if (order <= 4 && control_count < order + 2) || (order > 4 && control_count < 2 * order - 2) {
        return false;
    }

    let mut tolerance = (knots[order - 1] - knots[order - 3]).abs() * OPENNURBS_SQRT_EPSILON;
    tolerance =
        tolerance.max((knots[control_count - 1] - knots[order - 2]).abs() * OPENNURBS_SQRT_EPSILON);
    let right_start = control_count - order + 1;
    (0..2 * (order - 2)).all(|index| {
        let left_delta = knots[index + 1] - knots[index];
        let right_delta = knots[right_start + index + 1] - knots[right_start + index];
        (left_delta - right_delta).abs() <= tolerance
    })
}

pub(crate) fn de_boor<const DIMENSION: usize>(
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: Real,
    mut work: Vec<[Real; DIMENSION]>,
) -> Result<[Real; DIMENSION], GeometryError> {
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

pub(crate) fn interval_fraction(
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

fn blend_homogeneous<const DIMENSION: usize>(
    left: [Real; DIMENSION],
    right: [Real; DIMENSION],
    alpha: Real,
) -> Result<[Real; DIMENSION], GeometryError> {
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

fn blend_weighted_control_points(
    left: WeightedPoint3,
    right: WeightedPoint3,
    alpha: Real,
) -> Result<WeightedPoint3, GeometryError> {
    if alpha == 0.0 {
        return Ok(left);
    }
    if alpha == 1.0 {
        return Ok(right);
    }
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return Err(GeometryError::InvalidKnotVector {
            context: "knot-insertion blend factor is outside zero to one",
        });
    }

    // Work with weights normalized by their local maximum. This avoids the
    // overflow in `weight * coordinate` that a literal homogeneous blend can
    // encounter, while producing the identical projective control point.
    let scale = left.weight.max(right.weight);
    let left_weight = left.weight / scale;
    let right_weight = right.weight / scale;
    let complement = 1.0 - alpha;
    let normalized_weight = left_weight.mul_add(complement, right_weight * alpha);
    if !normalized_weight.is_finite() || normalized_weight <= 0.0 {
        return Err(GeometryError::NonFinite {
            context: "knot-insertion control weight",
        });
    }
    let weight = normalized_weight * scale;
    if !weight.is_finite() || weight <= 0.0 {
        return Err(GeometryError::NonFinite {
            context: "knot-insertion control weight",
        });
    }

    let right_fraction = ((right_weight * alpha) / normalized_weight).clamp(0.0, 1.0);
    let left_coordinates = left.point.to_array();
    let right_coordinates = right.point.to_array();
    let point: [Real; 3] = std::array::from_fn(|index| {
        left_coordinates[index].mul_add(
            1.0 - right_fraction,
            right_coordinates[index] * right_fraction,
        )
    });
    require_finite(point, "knot-insertion control point")?;
    Ok(WeightedPoint3 {
        point: Point3::try_new(point[0], point[1], point[2])?,
        weight,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tolerance;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn assert_point_near(actual: Point3, expected: Point3) {
        assert!(
            actual.is_near(
                expected,
                Tolerance::try_new(1.0e-12, 1.0e-12, 1.0e-12).unwrap()
            ),
            "actual {actual:?}, expected {expected:?}"
        );
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
    fn control_point_curve_matches_rhino_degree_lowering_and_domain() {
        let controls = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.5).unwrap(),
            Point3::try_new(4.0, -1.0, 2.0).unwrap(),
            Point3::try_new(4.5, 3.0, -0.5).unwrap(),
            Point3::try_new(10.0, 0.0, 1.0).unwrap(),
        ];
        let curve = NurbsCurve::try_control_point_curve(3, controls.clone()).unwrap();
        let domain_end = 17.976_753_701_093_052;
        assert_eq!(curve.degree(), 3);
        assert_eq!(
            curve
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            controls
        );
        for (actual, expected) in curve.knots().iter().zip([
            0.0,
            0.0,
            0.0,
            0.0,
            domain_end / 2.0,
            domain_end,
            domain_end,
            domain_end,
            domain_end,
        ]) {
            assert!((*actual - expected).abs() <= 2.0e-14);
        }

        let quadratic = NurbsCurve::try_control_point_curve(
            5,
            vec![point(0.0, 0.0), point(2.0, 3.0), point(10.0, 0.0)],
        )
        .unwrap();
        assert_eq!(quadratic.degree(), 2);
        assert_eq!(quadratic.domain(), 0.0..=13.0_f64.sqrt() + 73.0_f64.sqrt());

        assert!(matches!(
            NurbsCurve::try_control_point_curve(3, vec![point(1.0, 1.0), point(1.0, 1.0)]),
            Err(GeometryError::Degenerate {
                context: "control-point curve"
            })
        ));
    }

    #[test]
    fn smooth_control_point_curve_matches_rhino_periodic_layout() {
        let unique = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.5).unwrap(),
            Point3::try_new(4.0, -1.0, 2.0).unwrap(),
            Point3::try_new(4.5, 3.0, -0.5).unwrap(),
            Point3::try_new(10.0, 0.0, 1.0).unwrap(),
            Point3::try_new(8.0, -4.0, 2.0).unwrap(),
            Point3::try_new(2.0, -3.0, -1.0).unwrap(),
            Point3::try_new(-2.0, 1.0, 0.25).unwrap(),
        ];
        let curve = NurbsCurve::try_control_point_curve_with_closure(
            4,
            unique.clone(),
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(curve.degree(), 4);
        assert!(curve.is_periodic());
        assert!(curve.is_closed().unwrap());
        let expected_controls = unique[7..]
            .iter()
            .chain(&unique)
            .chain(&unique[..3])
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            curve
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            expected_controls
        );
        assert!((*curve.domain().end() - 46.426_262_339_780_31).abs() <= 2.0e-14);
        let short_knots = &curve.knots()[1..curve.knots().len() - 1];
        for (actual, expected) in short_knots.iter().zip([
            -17.409_848_377_417_617,
            -11.606_565_584_945_077,
            -5.803_282_792_472_538_5,
            0.0,
            5.803_282_792_472_538_5,
            11.606_565_584_945_077,
            17.409_848_377_417_617,
            23.213_131_169_890_154,
            29.016_413_962_362_69,
            34.819_696_754_835_235,
            40.622_979_547_307_77,
            46.426_262_339_780_31,
            52.229_545_132_252_845,
            58.032_827_924_725_38,
            63.836_110_717_197_926,
        ]) {
            assert!((*actual - expected).abs() <= 3.0e-14);
        }

        let lowered = NurbsCurve::try_control_point_curve_with_closure(
            11,
            unique.clone(),
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(lowered.degree(), 8);
        assert_eq!(lowered.control_points().len(), 16);
        assert_eq!(lowered.control_points()[0].point(), unique[5]);
        assert!(lowered.is_periodic());
        assert!((*lowered.domain().end() - 70.187_373_289_648_95).abs() <= 3.0e-14);
    }

    #[test]
    fn closed_control_point_curves_match_rhino_degree_lowering_and_seams() {
        let points = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 1.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        ];
        let smooth = NurbsCurve::try_control_point_curve_with_closure(
            5,
            points.clone(),
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(smooth.degree(), 3);
        assert!(smooth.is_periodic());
        assert_eq!(
            smooth
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![
                points[2], points[0], points[1], points[2], points[0], points[1]
            ]
        );
        assert!((*smooth.domain().end() - 36.085_640_040_590_51).abs() <= 2.0e-14);

        let sharp = NurbsCurve::try_control_point_curve_with_closure(
            5,
            points.clone(),
            ControlPointCurveClosure::Sharp,
        )
        .unwrap();
        assert_eq!(sharp.degree(), 3);
        assert!(!sharp.is_periodic());
        assert!(sharp.is_closed().unwrap());
        assert_eq!(
            sharp
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![points[0], points[1], points[2], points[0]]
        );
        assert!((*sharp.domain().end() - 22.343_982_653_816_568).abs() <= 2.0e-14);

        let linear = NurbsCurve::try_control_point_curve_with_closure(
            1,
            points.clone(),
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(linear.degree(), 1);
        assert!(!linear.is_periodic());
        assert!(linear.is_closed().unwrap());

        let mut explicitly_closed = points.clone();
        explicitly_closed.push(points[0]);
        let normalized = NurbsCurve::try_control_point_curve_with_closure(
            3,
            explicitly_closed,
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(normalized, smooth);

        assert_eq!(
            NurbsCurve::try_control_point_curve_with_closure(
                3,
                points[..2].to_vec(),
                ControlPointCurveClosure::Sharp,
            ),
            Err(GeometryError::InsufficientClosedControlPoints { actual: 2 })
        );
    }

    #[test]
    fn control_polygon_uses_rhino_periodic_window_and_rejects_zero_segments() {
        let points = vec![
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(3.0, 2.0),
            point(1.0, 4.0),
            point(-1.0, 2.0),
        ];
        let periodic = NurbsCurve::try_control_point_curve_with_closure(
            3,
            points.clone(),
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(
            periodic
                .control_polygon(Tolerance::DEFAULT)
                .unwrap()
                .vertices(),
            &[
                points[0], points[1], points[2], points[3], points[4], points[0]
            ]
        );

        let degree_two = NurbsCurve::try_new(
            2,
            vec![points[0], points[1], points[2], points[0], points[1]],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        assert_eq!(
            degree_two
                .control_polygon(Tolerance::DEFAULT)
                .unwrap()
                .vertices(),
            &[points[1], points[2], points[0], points[1]]
        );

        let repeated = NurbsCurve::try_new(
            3,
            vec![points[0], points[0], points[2], points[3], points[4]],
            vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(
            repeated.control_polygon(Tolerance::DEFAULT),
            Err(GeometryError::DegeneratePolylineSegment { segment: 0 })
        );
    }

    #[test]
    fn closed_state_matches_opennurbs_endpoint_and_size_rules() {
        let closed = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(3.0, 0.0),
                point(3.0, 2.0),
                point(0.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
        )
        .unwrap();
        assert!(closed.is_closed().unwrap());
        assert_eq!(
            closed.extract_point_locations().unwrap(),
            vec![point(0.0, 0.0), point(3.0, 0.0), point(3.0, 2.0)]
        );

        let nearly_closed = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(3.0, 0.0),
                point(3.0, 2.0),
                point(1.0e-10, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
        )
        .unwrap();
        assert!(nearly_closed.is_closed().unwrap());
        assert_eq!(
            nearly_closed.extract_point_locations().unwrap(),
            vec![
                point(0.0, 0.0),
                point(3.0, 0.0),
                point(3.0, 2.0),
                point(1.0e-10, 0.0),
            ]
        );

        let open = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(3.0, 0.0),
                point(3.0, 2.0),
                point(1.0e-6, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
        )
        .unwrap();
        assert!(!open.is_closed().unwrap());

        let two_segment_loop = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(3.0, 0.0), point(0.0, 0.0)],
            vec![0.0, 0.0, 1.0, 2.0, 2.0],
        )
        .unwrap();
        assert!(!two_segment_loop.is_closed().unwrap());

        let periodic = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(1.0, 2.0),
                point(0.0, 0.0),
                point(2.0, 0.0),
            ],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        assert!(periodic.is_periodic());
        assert!(periodic.is_closed().unwrap());
        assert_eq!(
            periodic.extract_point_locations().unwrap(),
            vec![point(0.0, 0.0), point(2.0, 0.0), point(1.0, 2.0)]
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
    fn reversal_negates_the_full_domain_and_parameter_direction() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(1.0, 3.0),
                point(4.0, -1.0),
                point(7.0, 2.0),
            ],
            vec![-2.0, -2.0, -2.0, 1.0, 5.0, 5.0, 5.0],
        )
        .unwrap();
        let reversed = curve.reversed().unwrap();
        assert_eq!(reversed.domain(), -5.0..=2.0);
        assert_eq!(reversed.knots(), &[-5.0, -5.0, -5.0, -1.0, 2.0, 2.0, 2.0]);
        for sample in 0..=16 {
            let normalized = sample as Real / 16.0;
            let reversed_parameter = reversed.parameter_at(normalized).unwrap();
            let original_parameter = curve.parameter_at(1.0 - normalized).unwrap();
            assert_point_near(
                reversed.evaluate(reversed_parameter).unwrap(),
                curve.evaluate(original_parameter).unwrap(),
            );
            let actual = reversed.derivative_at(reversed_parameter).unwrap();
            let expected = curve.derivative_at(original_parameter).unwrap();
            assert!(Tolerance::DEFAULT.approx_eq(actual.x(), -expected.x()));
            assert!(Tolerance::DEFAULT.approx_eq(actual.y(), -expected.y()));
            assert!(Tolerance::DEFAULT.approx_eq(actual.z(), -expected.z()));
        }
        assert_eq!(reversed.reversed().unwrap(), curve);
    }

    #[test]
    fn reparameterization_preserves_shape_and_maps_the_full_knot_vector() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                point(-2.0, 1.0),
                point(0.0, 3.0),
                point(2.0, -1.0),
                point(4.0, 2.0),
                point(7.0, 0.0),
            ],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        let mapped = curve.try_reparameterized(10.0..=16.0).unwrap();
        assert_eq!(mapped.domain(), 10.0..=16.0);
        assert_eq!(
            mapped.knots(),
            &[6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0]
        );
        for sample in 0..=32 {
            let normalized = sample as Real / 32.0;
            assert_point_near(
                curve
                    .evaluate(curve.parameter_at(normalized).unwrap())
                    .unwrap(),
                mapped
                    .evaluate(mapped.parameter_at(normalized).unwrap())
                    .unwrap(),
            );
        }

        for domain in [
            1.0..=1.0,
            2.0..=-1.0,
            Real::NEG_INFINITY..=1.0,
            0.0..=Real::NAN,
        ] {
            assert!(mapped.try_reparameterized(domain).is_err());
        }
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
        assert!(
            Tolerance::try_new(1.0e-11, 1.0e-12, 1.0e-12)
                .unwrap()
                .approx_eq(
                    curve.length(Tolerance::DEFAULT).unwrap(),
                    std::f64::consts::FRAC_PI_2
                )
        );
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

    #[test]
    fn knot_insertion_preserves_a_rational_nonuniform_curve() {
        let curve = NurbsCurve::try_new_rational(
            3,
            vec![
                WeightedPoint3::try_new(point(-3.0, 1.0), 0.75).unwrap(),
                WeightedPoint3::try_new(point(-1.0, 4.0), 2.0).unwrap(),
                WeightedPoint3::try_new(point(2.0, -2.0), 0.4).unwrap(),
                WeightedPoint3::try_new(point(4.0, 3.0), 3.5).unwrap(),
                WeightedPoint3::try_new(point(7.0, -1.0), 1.25).unwrap(),
                WeightedPoint3::try_new(point(9.0, 2.0), 0.9).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 0.35, 0.8, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let refined = curve.try_insert_knot(0.52, 3).unwrap();

        assert_eq!(refined.degree(), curve.degree());
        assert_eq!(refined.domain(), curve.domain());
        assert_eq!(refined.knot_multiplicity(0.52).unwrap(), 3);
        assert_eq!(
            refined.control_points().len(),
            curve.control_points().len() + 3
        );
        for sample in 0..=64 {
            let parameter = sample as Real / 64.0;
            assert_point_near(
                refined.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
            let actual = refined.derivative_at(parameter).unwrap();
            let expected = curve.derivative_at(parameter).unwrap();
            assert!(Tolerance::DEFAULT.approx_eq(actual.x(), expected.x()));
            assert!(Tolerance::DEFAULT.approx_eq(actual.y(), expected.y()));
            assert!(Tolerance::DEFAULT.approx_eq(actual.z(), expected.z()));
        }

        let fully_refined = refined.try_insert_knot(0.52, 4).unwrap();
        assert_eq!(fully_refined.knot_multiplicity(0.52).unwrap(), 4);
        for parameter in [0.0, 0.2, 0.52, 0.7, 1.0] {
            assert_point_near(
                fully_refined.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }
        let actual = fully_refined.derivative_at(0.52).unwrap();
        let expected = curve.derivative_at(0.52).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(actual.x(), expected.x()));
        assert!(Tolerance::DEFAULT.approx_eq(actual.y(), expected.y()));
        assert!(Tolerance::DEFAULT.approx_eq(actual.z(), expected.z()));
        assert_eq!(
            fully_refined.try_insert_knot(0.52, 2).unwrap(),
            fully_refined
        );
    }

    #[test]
    fn endpoint_refinement_preserves_nonclamped_curve_evaluation() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                point(-2.0, 1.0),
                point(0.0, 4.0),
                point(5.0, -2.0),
                point(8.0, 3.0),
            ],
            vec![-2.0, -1.0, 0.0, 0.8, 2.0, 3.0, 4.0],
        )
        .unwrap();

        for parameter in [0.0, 2.0] {
            let refined = curve.try_insert_knot(parameter, 3).unwrap();
            assert_eq!(refined.knot_multiplicity(parameter).unwrap(), 3);
            for sample in 0..=32 {
                let t = sample as Real / 16.0;
                let actual = refined.evaluate(t).unwrap();
                let expected = curve.evaluate(t).unwrap();
                assert!(
                    actual.is_near(
                        expected,
                        Tolerance::try_new(1.0e-12, 1.0e-12, 1.0e-12).unwrap()
                    ),
                    "refined endpoint {parameter}, sample {t}: actual {actual:?}, expected {expected:?}"
                );
            }
        }
    }

    #[test]
    fn split_preserves_rational_curve_parameters_and_derivatives() {
        let middle_weight = 0.5_f64.sqrt();
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0), middle_weight).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let (left, right) = curve.try_split(0.4).unwrap();

        assert_eq!(left.domain(), 0.0..=0.4);
        assert_eq!(right.domain(), 0.4..=1.0);
        assert!(
            left.knots()[left.knots().len() - 3..]
                .iter()
                .all(|knot| *knot == 0.4)
        );
        assert!(right.knots()[..3].iter().all(|knot| *knot == 0.4));
        assert_point_near(left.evaluate(0.4).unwrap(), right.evaluate(0.4).unwrap());
        for (piece, start, end) in [(&left, 0.0_f64, 0.4_f64), (&right, 0.4, 1.0)] {
            for sample in 0..=16 {
                let fraction = sample as Real / 16.0;
                let parameter = start.mul_add(1.0 - fraction, end * fraction);
                assert_point_near(
                    piece.evaluate(parameter).unwrap(),
                    curve.evaluate(parameter).unwrap(),
                );
                let actual = piece.derivative_at(parameter).unwrap();
                let expected = curve.derivative_at(parameter).unwrap();
                assert!(Tolerance::DEFAULT.approx_eq(actual.x(), expected.x()));
                assert!(Tolerance::DEFAULT.approx_eq(actual.y(), expected.y()));
                assert!(Tolerance::DEFAULT.approx_eq(actual.z(), expected.z()));
            }
        }
    }

    #[test]
    fn split_reuses_degree_multiplicity_control_and_preserves_full_break() {
        let continuous = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(1.0, 3.0),
                point(2.0, 1.0),
                point(5.0, -2.0),
                point(8.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let (left, right) = continuous.try_split(0.5).unwrap();
        assert_eq!(left.control_points().len(), 3);
        assert_eq!(right.control_points().len(), 3);
        assert_eq!(left.control_points()[2], right.control_points()[0]);
        assert_point_near(
            left.evaluate(0.5).unwrap(),
            continuous.evaluate(0.5).unwrap(),
        );
        assert_point_near(
            right.evaluate(0.5).unwrap(),
            continuous.evaluate(0.5).unwrap(),
        );

        let discontinuous = NurbsCurve::try_new(
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
        let (left, right) = discontinuous.try_split(0.5).unwrap();
        assert_eq!(left.control_points().len(), 2);
        assert_eq!(right.control_points().len(), 2);
        assert_eq!(left.evaluate(0.5).unwrap(), point(1.0, 0.0));
        assert_eq!(right.evaluate(0.5).unwrap(), point(10.0, 0.0));
        assert_ne!(left.control_points()[1], right.control_points()[0]);
    }

    #[test]
    fn partial_trim_is_exact_and_clamps_nonclamped_ends() {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(-2.0, 1.0), 0.75).unwrap(),
                WeightedPoint3::try_new(point(0.0, 4.0), 2.0).unwrap(),
                WeightedPoint3::try_new(point(5.0, -2.0), 0.5).unwrap(),
                WeightedPoint3::try_new(point(8.0, 3.0), 1.25).unwrap(),
            ],
            vec![-2.0, -1.0, 0.0, 0.8, 2.0, 3.0, 4.0],
        )
        .unwrap();
        let trimmed = curve.try_trimmed(0.25..=1.6).unwrap();

        assert_eq!(trimmed.domain(), 0.25..=1.6);
        assert!(trimmed.knots()[..3].iter().all(|knot| *knot == 0.25));
        assert!(
            trimmed.knots()[trimmed.knots().len() - 3..]
                .iter()
                .all(|knot| *knot == 1.6)
        );
        for sample in 0..=32 {
            let fraction = sample as Real / 32.0;
            let parameter = 0.25_f64.mul_add(1.0 - fraction, 1.6 * fraction);
            assert_point_near(
                trimmed.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }

        let from_start = curve.try_trimmed(0.0..=1.6).unwrap();
        assert!(from_start.knots()[..3].iter().all(|knot| *knot == 0.0));
        assert_point_near(
            from_start.evaluate(0.0).unwrap(),
            curve.evaluate(0.0).unwrap(),
        );
        assert_eq!(curve.try_trimmed(curve.domain()).unwrap(), curve);
    }

    #[test]
    fn splitting_periodic_curve_opens_and_clamps_both_pieces() {
        let curve = NurbsCurve::try_control_point_curve_with_closure(
            3,
            vec![
                point(-3.0, 0.0),
                point(-1.0, 3.0),
                point(2.0, 4.0),
                point(5.0, 1.0),
                point(4.0, -3.0),
                point(0.0, -4.0),
            ],
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        let domain = curve.domain();
        assert_eq!(curve.try_trimmed(domain.clone()).unwrap(), curve);
        let split = curve.parameter_at(0.43).unwrap();
        let (left, right) = curve.try_split(split).unwrap();

        assert!(!left.is_periodic());
        assert!(!right.is_periodic());
        assert!(
            left.knots()[..4]
                .iter()
                .all(|knot| *knot == *domain.start())
        );
        assert!(
            left.knots()[left.knots().len() - 4..]
                .iter()
                .all(|knot| *knot == split)
        );
        assert!(right.knots()[..4].iter().all(|knot| *knot == split));
        assert!(
            right.knots()[right.knots().len() - 4..]
                .iter()
                .all(|knot| *knot == *domain.end())
        );
        for sample in 0..=40 {
            let parameter = curve.parameter_at(sample as Real / 40.0).unwrap();
            let piece = if parameter <= split { &left } else { &right };
            assert_point_near(
                piece.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }
        assert_point_near(
            left.evaluate(*domain.start()).unwrap(),
            right.evaluate(*domain.end()).unwrap(),
        );
    }

    #[test]
    fn refinement_handles_large_coordinates_and_weight_ranges() {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(Point3::try_new(-1.0e300, 2.0e299, 0.0).unwrap(), 1.0e-100)
                    .unwrap(),
                WeightedPoint3::try_new(Point3::try_new(8.0e299, -4.0e299, 0.0).unwrap(), 1.0e100)
                    .unwrap(),
                WeightedPoint3::try_new(Point3::try_new(1.0e300, 6.0e299, 0.0).unwrap(), 2.0e99)
                    .unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let refined = curve.try_insert_knot(0.5, 2).unwrap();
        for parameter in [0.0, 0.2, 0.5, 0.8, 1.0] {
            let actual = refined.evaluate(parameter).unwrap().to_array();
            let expected = curve.evaluate(parameter).unwrap().to_array();
            assert!(
                actual
                    .into_iter()
                    .zip(expected)
                    .all(|(actual, expected)| Tolerance::DEFAULT.approx_eq(actual, expected))
            );
        }
        assert!(refined.control_points().iter().all(|control| {
            control.weight().is_finite()
                && control.point().to_array().into_iter().all(Real::is_finite)
        }));
    }

    #[test]
    fn refinement_split_and_trim_reject_invalid_requests() {
        let curve = NurbsCurve::try_clamped_uniform(
            2,
            vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 0.0)],
        )
        .unwrap();
        assert_eq!(
            curve.try_insert_knot(0.5, 0),
            Err(GeometryError::InvalidKnotMultiplicity {
                actual: 0,
                maximum: 3
            })
        );
        assert!(matches!(
            curve.try_insert_knot(0.5, 4),
            Err(GeometryError::InvalidKnotMultiplicity { .. })
        ));
        assert!(matches!(
            curve.try_insert_knot(-0.1, 1),
            Err(GeometryError::ParameterOutOfDomain { .. })
        ));
        assert!(curve.try_insert_knot(Real::NAN, 1).is_err());
        for parameter in [-1.0, 0.0, 1.0, 2.0, Real::NAN] {
            assert!(curve.try_split(parameter).is_err());
        }
        for interval in [
            0.5..=0.5,
            0.8..=0.2,
            -0.1..=0.8,
            0.2..=1.1,
            Real::NAN..=0.5,
            0.2..=Real::INFINITY,
        ] {
            assert_eq!(
                curve.try_trimmed(interval),
                Err(GeometryError::InvalidCurveTrimInterval)
            );
        }
    }
}
