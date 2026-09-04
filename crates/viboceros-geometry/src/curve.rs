use std::f64::consts::FRAC_PI_2;

use crate::{
    Circle3, CircularArc3, CurveEvaluationSide, Ellipse3, GeometryError, LineSegment, NurbsCurve,
    Point3, PolyCurve3, Polyline3, Real, Tolerance, UnitVector3, Vector3,
    integration::integrate_adaptive,
    nurbs::{CURVE_COINCIDENCE_ABSOLUTE, curve_points_coincident},
    require_finite,
};

/// Allocation guard for commands that create arc-length division points.
pub const MAX_CURVE_DIVISION_POINTS: usize = 1_000_000;

/// A point on a curve paired with its natural parameter and unit tangent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveSample {
    parameter: Real,
    point: Point3,
    tangent: UnitVector3,
}

impl CurveSample {
    #[inline]
    pub const fn parameter(self) -> Real {
        self.parameter
    }

    #[inline]
    pub const fn point(self) -> Point3 {
        self.point
    }

    #[inline]
    pub const fn tangent(self) -> UnitVector3 {
        self.tangent
    }

    /// Keeps the sampled location while reversing the curve direction.
    #[inline]
    pub fn reversed_direction(self) -> Self {
        Self {
            tangent: self.tangent.opposite(),
            ..self
        }
    }
}

/// A borrowed reference to any curve representation supported by the core.
#[derive(Clone, Copy, Debug)]
pub enum CurveRef<'a> {
    Line(&'a LineSegment),
    Circle(&'a Circle3),
    Arc(&'a CircularArc3),
    Ellipse(&'a Ellipse3),
    Polyline(&'a Polyline3),
    NurbsCurve(&'a NurbsCurve),
    PolyCurve(&'a PolyCurve3),
}

impl CurveRef<'_> {
    /// Computes the complete curve length with controlled numerical accuracy.
    ///
    /// Exact analytic and piecewise-linear representations use their direct
    /// formulas. Ellipse and NURBS representations use the supplied absolute
    /// and relative tolerances to control adaptive integration.
    pub fn length(self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        match self {
            Self::Line(line) => line.length(),
            Self::Circle(circle) => circle.length(),
            Self::Arc(arc) => arc.length(),
            Self::Ellipse(ellipse) => ellipse.length(tolerance),
            Self::Polyline(polyline) => polyline.length(),
            Self::NurbsCurve(curve) => curve.length(tolerance),
            Self::PolyCurve(curve) => curve.length(tolerance),
        }
    }

    /// Returns whether the curve has coincident natural endpoints.
    pub fn is_closed(self) -> Result<bool, GeometryError> {
        Ok(match self {
            Self::Circle(_) | Self::Ellipse(_) => true,
            Self::Line(_) | Self::Arc(_) => false,
            Self::Polyline(polyline) => polyline.is_closed(),
            Self::NurbsCurve(curve) => curve.is_closed()?,
            Self::PolyCurve(curve) => curve.is_closed()?,
        })
    }

    /// Tests whether the complete curve lies in a plane within the document's
    /// absolute tolerance.
    ///
    /// Analytic curves carry an exact plane. Polyline and NURBS predicates
    /// follow OpenNURBS' largest-control-triangle test and therefore retain
    /// Rhino's behavior for degenerate, reversed, and unclamped curves.
    pub fn is_planar(self, tolerance: Tolerance) -> Result<bool, GeometryError> {
        match self {
            Self::Line(_) | Self::Circle(_) | Self::Arc(_) | Self::Ellipse(_) => Ok(true),
            Self::Polyline(polyline) => control_polygon_is_planar(
                polyline.vertices().len(),
                |index| polyline.vertices()[index],
                polyline.vertices()[0],
                tolerance,
            ),
            Self::NurbsCurve(curve) => curve.is_planar(tolerance),
            Self::PolyCurve(curve) => {
                let controls = curve
                    .segments()
                    .iter()
                    .flat_map(|s| s.control_points())
                    .map(|c| c.point())
                    .collect::<Vec<_>>();
                control_polygon_is_planar(
                    controls.len(),
                    |index| controls[index],
                    curve.evaluate(*curve.domain().start())?,
                    tolerance,
                )
            }
        }
    }

    pub fn start_point(self) -> Result<Point3, GeometryError> {
        match self {
            Self::Line(line) => Ok(line.start()),
            Self::Circle(circle) => circle.point_at_angle(0.0),
            Self::Arc(arc) => arc.start(),
            Self::Ellipse(ellipse) => ellipse.point_at_angle(0.0),
            Self::Polyline(polyline) => Ok(polyline.vertices()[0]),
            Self::NurbsCurve(curve) => curve.evaluate(*curve.domain().start()),
            Self::PolyCurve(curve) => curve.evaluate(*curve.domain().start()),
        }
    }

    pub fn end_point(self) -> Result<Point3, GeometryError> {
        match self {
            Self::Line(line) => Ok(line.end()),
            Self::Circle(circle) => circle.point_at_angle(0.0),
            Self::Arc(arc) => arc.end(),
            Self::Ellipse(ellipse) => ellipse.point_at_angle(0.0),
            Self::Polyline(polyline) => Ok(*polyline
                .vertices()
                .last()
                .expect("a validated polyline has vertices")),
            Self::NurbsCurve(curve) => curve.evaluate(*curve.domain().end()),
            Self::PolyCurve(curve) => curve.evaluate(*curve.domain().end()),
        }
    }

    /// Divides the curve into `segment_count` equal arc-length segments.
    ///
    /// `include_ends` includes both open endpoints, or a single seam on a closed
    /// curve. Otherwise only the `segment_count - 1` interior stations are
    /// returned, matching RhinoCommon's DivideByCount.
    pub fn divide_by_count(
        self,
        segment_count: usize,
        include_ends: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<Point3>, GeometryError> {
        let mut points = self.sample_equal_length_points(segment_count, include_ends, tolerance)?;
        if !include_ends || self.is_closed()? {
            points.pop();
        }
        Ok(points)
    }

    /// Samples every interval boundary, including the end even for a closed
    /// curve. Useful for algorithms needing an explicit repeated closure point;
    /// user-facing division should use [`Self::divide_by_count`] instead.
    pub fn sample_equal_length_points(
        self,
        segment_count: usize,
        include_start: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<Point3>, GeometryError> {
        self.divide_by_count_impl(segment_count, include_start, tolerance, None)
    }

    /// Uses RhinoCommon's fixed fractional length tolerance for the sampling
    /// stage behind `TweenCurves`, retaining an explicit interpolation seam.
    pub(crate) fn divide_by_count_for_tween(
        self,
        segment_count: usize,
        include_start: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<Point3>, GeometryError> {
        self.divide_by_count_impl(segment_count, include_start, tolerance, Some(1.0e-8))
    }

    fn divide_by_count_impl(
        self,
        segment_count: usize,
        include_start: bool,
        tolerance: Tolerance,
        fractional_tolerance: Option<Real>,
    ) -> Result<Vec<Point3>, GeometryError> {
        if segment_count == 0 {
            return Err(GeometryError::InvalidCurveDivisionCount {
                actual: segment_count,
                maximum: MAX_CURVE_DIVISION_POINTS,
            });
        }
        let point_count = segment_count
            .checked_add(usize::from(include_start))
            .ok_or(GeometryError::InvalidCurveDivisionCount {
                actual: segment_count,
                maximum: MAX_CURVE_DIVISION_POINTS,
            })?;
        require_division_capacity(point_count)?;

        let sampler = ArcLengthSampler::try_new(self, tolerance)?;
        let first_index = usize::from(!include_start);
        let mut points = Vec::with_capacity(point_count);
        for index in first_index..=segment_count {
            let distance = if index == segment_count {
                sampler.total_length
            } else {
                sampler.total_length * (index as Real / segment_count as Real)
            };
            points.push(match fractional_tolerance {
                Some(fractional_tolerance) => sampler
                    .point_at_distance_with_fractional_tolerance(distance, fractional_tolerance)?,
                None => sampler.point_at_distance(distance)?,
            });
        }
        Ok(points)
    }

    /// Returns points separated by the requested arc length.
    ///
    /// The final natural endpoint is returned only when it lies on an exact
    /// division. `include_start` controls whether the natural start is
    /// returned.
    pub fn divide_by_length(
        self,
        segment_length: Real,
        include_start: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<Point3>, GeometryError> {
        require_finite([segment_length], "curve division length")?;
        if segment_length <= 0.0 {
            return Err(GeometryError::InvalidCurveDivisionLength);
        }
        let sampler = ArcLengthSampler::try_new(self, tolerance)?;
        let quotient = (sampler.total_length / segment_length).floor();
        if !quotient.is_finite() || quotient > MAX_CURVE_DIVISION_POINTS as Real {
            return Err(GeometryError::TooManyCurveDivisionPoints {
                maximum: MAX_CURVE_DIVISION_POINTS,
            });
        }
        let division_count = quotient as usize;
        let requested_capacity = division_count
            .checked_add(usize::from(include_start))
            .ok_or(GeometryError::TooManyCurveDivisionPoints {
                maximum: MAX_CURVE_DIVISION_POINTS,
            })?;
        require_division_capacity(requested_capacity)?;

        let mut points = Vec::with_capacity(requested_capacity);
        if include_start {
            points.push(sampler.point_at_distance(0.0)?);
        }
        for index in 1..=division_count {
            let mut distance = segment_length * index as Real;
            require_finite([distance], "curve division distance")?;
            if distance > sampler.total_length {
                if tolerance.approx_eq(distance, sampler.total_length) {
                    distance = sampler.total_length;
                } else {
                    break;
                }
            }
            points.push(sampler.point_at_distance(distance)?);
        }
        Ok(points)
    }

    /// Divides by equal arc length and returns each point's unit tangent.
    pub fn divide_by_count_samples(
        self,
        segment_count: usize,
        include_start: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<CurveSample>, GeometryError> {
        if segment_count == 0 {
            return Err(GeometryError::InvalidCurveDivisionCount {
                actual: segment_count,
                maximum: MAX_CURVE_DIVISION_POINTS,
            });
        }
        let sample_count = segment_count
            .checked_add(usize::from(include_start))
            .ok_or(GeometryError::InvalidCurveDivisionCount {
                actual: segment_count,
                maximum: MAX_CURVE_DIVISION_POINTS,
            })?;
        require_division_capacity(sample_count)?;

        let sampler = ArcLengthSampler::try_new(self, tolerance)?;
        let first_index = usize::from(!include_start);
        let mut samples = Vec::with_capacity(sample_count);
        for index in first_index..=segment_count {
            let distance = if index == segment_count {
                sampler.total_length
            } else {
                sampler.total_length * (index as Real / segment_count as Real)
            };
            samples.push(sampler.sample_at_distance(distance)?);
        }
        Ok(samples)
    }

    /// Returns the natural start point and its forward unit tangent.
    pub fn start_sample(self, tolerance: Tolerance) -> Result<CurveSample, GeometryError> {
        ArcLengthSampler::try_new(self, tolerance)?.sample_at_distance(0.0)
    }

    /// Samples at a fixed arc-length interval and returns unit tangents.
    pub fn divide_by_length_samples(
        self,
        segment_length: Real,
        include_start: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<CurveSample>, GeometryError> {
        require_finite([segment_length], "curve division length")?;
        if segment_length <= 0.0 {
            return Err(GeometryError::InvalidCurveDivisionLength);
        }
        let sampler = ArcLengthSampler::try_new(self, tolerance)?;
        let quotient = (sampler.total_length / segment_length).floor();
        if !quotient.is_finite() || quotient > MAX_CURVE_DIVISION_POINTS as Real {
            return Err(GeometryError::TooManyCurveDivisionPoints {
                maximum: MAX_CURVE_DIVISION_POINTS,
            });
        }
        let division_count = quotient as usize;
        let requested_capacity = division_count
            .checked_add(usize::from(include_start))
            .ok_or(GeometryError::TooManyCurveDivisionPoints {
                maximum: MAX_CURVE_DIVISION_POINTS,
            })?;
        require_division_capacity(requested_capacity)?;

        let mut samples = Vec::with_capacity(requested_capacity);
        if include_start {
            samples.push(sampler.sample_at_distance(0.0)?);
        }
        for index in 1..=division_count {
            let mut distance = segment_length * index as Real;
            require_finite([distance], "curve division distance")?;
            if distance > sampler.total_length {
                if tolerance.approx_eq(distance, sampler.total_length) {
                    distance = sampler.total_length;
                } else {
                    break;
                }
            }
            samples.push(sampler.sample_at_distance(distance)?);
        }
        Ok(samples)
    }

    /// Evaluates a point and a unit tangent in the natural curve domain.
    pub fn evaluate_with_tangent(self, parameter: Real) -> Result<CurveSample, GeometryError> {
        let point = match self {
            Self::Line(line) => line.point_at(parameter)?,
            Self::Circle(circle) => {
                require_periodic_parameter(parameter, std::f64::consts::TAU)?;
                circle.point_at_angle(parameter)?
            }
            Self::Arc(arc) => arc.point_at(parameter)?,
            Self::Ellipse(ellipse) => {
                require_periodic_parameter(parameter, std::f64::consts::TAU)?;
                ellipse.point_at_angle(parameter)?
            }
            Self::Polyline(polyline) => {
                let end = polyline.segment_count() as Real;
                if !(0.0..=end).contains(&parameter) {
                    return Err(GeometryError::ParameterOutOfDomain {
                        parameter,
                        domain_start: 0.0,
                        domain_end: end,
                    });
                }
                if parameter == end {
                    *polyline.vertices().last().expect("a polyline has vertices")
                } else {
                    let index = parameter.floor() as usize;
                    LineSegment::from_validated(
                        polyline.vertices()[index],
                        polyline.vertices()[index + 1],
                    )
                    .point_at(parameter - index as Real)?
                }
            }
            Self::NurbsCurve(curve) => curve.evaluate(parameter)?,
            Self::PolyCurve(curve) => curve.evaluate(parameter)?,
        };
        let derivative = match self {
            Self::Line(line) => line.start().vector_to(line.end())?,
            Self::Circle(circle) => periodic_derivative(
                circle.x_axis().as_vector(),
                circle.y_axis().as_vector(),
                circle.radius(),
                circle.radius(),
                parameter,
            )?,
            Self::Arc(arc) => arc
                .normal()?
                .as_vector()
                .cross(arc.center().vector_to(point)?)?,
            Self::Ellipse(ellipse) => periodic_derivative(
                ellipse.x_axis().as_vector(),
                ellipse.y_axis().as_vector(),
                ellipse.radius_x(),
                ellipse.radius_y(),
                parameter,
            )?,
            Self::Polyline(polyline) => {
                let index = (parameter.floor() as usize).min(polyline.segment_count() - 1);
                polyline.vertices()[index].vector_to(polyline.vertices()[index + 1])?
            }
            Self::NurbsCurve(curve) => curve.derivative_at(parameter)?,
            Self::PolyCurve(curve) => curve.evaluate_with_derivative(parameter)?.1,
        };
        Ok(CurveSample {
            parameter,
            point,
            // A derivative's magnitude depends on parameter scaling, so model
            // distance tolerance must not decide whether its direction exists.
            tangent: derivative.normalized_nonzero()?,
        })
    }

    /// Returns the derivative of the unit tangent with respect to arc length.
    ///
    /// This is the curvature vector, including its model-space direction. At
    /// the interior of a polyline segment it is zero; vertices have no unique
    /// curvature and deterministically use the active segment's zero value.
    pub(crate) fn curvature_vector(self, parameter: Real) -> Result<Vector3, GeometryError> {
        let point = self.evaluate_with_tangent(parameter)?.point();
        match self {
            Self::Line(_) | Self::Polyline(_) => Vector3::try_new(0.0, 0.0, 0.0),
            Self::Circle(circle) => radial_curvature(point, circle.center(), circle.radius()),
            Self::Arc(arc) => radial_curvature(point, arc.center(), arc.radius()),
            Self::Ellipse(ellipse) => {
                let (sine, cosine) = parameter.sin_cos();
                let first = combine_vectors(
                    ellipse.x_axis().as_vector(),
                    ellipse.y_axis().as_vector(),
                    -ellipse.radius_x() * sine,
                    ellipse.radius_y() * cosine,
                )?;
                let second = combine_vectors(
                    ellipse.x_axis().as_vector(),
                    ellipse.y_axis().as_vector(),
                    -ellipse.radius_x() * cosine,
                    -ellipse.radius_y() * sine,
                )?;
                curvature_from_derivatives(first, second)
            }
            Self::NurbsCurve(curve) => {
                let (_, first, second) = curve.evaluate_with_second_derivative(parameter)?;
                curvature_from_derivatives(first, second)
            }
            Self::PolyCurve(curve) => {
                let (_, first, second) =
                    curve.evaluate_with_second_derivative(parameter, CurveEvaluationSide::Right)?;
                curvature_from_derivatives(first, second)
            }
        }
    }
}

fn radial_curvature(point: Point3, center: Point3, radius: Real) -> Result<Vector3, GeometryError> {
    point
        .vector_to(center)?
        .normalized_nonzero()?
        .as_vector()
        .scaled(1.0 / radius)
}

fn combine_vectors(
    first: Vector3,
    second: Vector3,
    first_scale: Real,
    second_scale: Real,
) -> Result<Vector3, GeometryError> {
    let first = first.to_array();
    let second = second.to_array();
    Vector3::try_new(
        first_scale.mul_add(first[0], second_scale * second[0]),
        first_scale.mul_add(first[1], second_scale * second[1]),
        first_scale.mul_add(first[2], second_scale * second[2]),
    )
}

fn curvature_from_derivatives(first: Vector3, second: Vector3) -> Result<Vector3, GeometryError> {
    let speed = first.length()?;
    if speed == 0.0 {
        return Err(GeometryError::Degenerate {
            context: "curve curvature",
        });
    }
    let tangent = first.normalized_nonzero()?;
    let tangential = tangent
        .as_vector()
        .scaled(tangent.as_vector().dot(second)?)?;
    Vector3::try_new(
        second.x() - tangential.x(),
        second.y() - tangential.y(),
        second.z() - tangential.z(),
    )?
    .scaled(1.0 / speed)?
    .scaled(1.0 / speed)
}

fn require_periodic_parameter(parameter: Real, domain_end: Real) -> Result<(), GeometryError> {
    require_finite([parameter], "curve parameter")?;
    if (0.0..=domain_end).contains(&parameter) {
        Ok(())
    } else {
        Err(GeometryError::ParameterOutOfDomain {
            parameter,
            domain_start: 0.0,
            domain_end,
        })
    }
}

fn periodic_derivative(
    x_axis: Vector3,
    y_axis: Vector3,
    radius_x: Real,
    radius_y: Real,
    parameter: Real,
) -> Result<Vector3, GeometryError> {
    let (sine, cosine) = parameter.sin_cos();
    let x = x_axis.scaled(-radius_x * sine)?;
    let y = y_axis.scaled(radius_y * cosine)?;
    Vector3::try_new(x.x() + y.x(), x.y() + y.y(), x.z() + y.z())
}

impl NurbsCurve {
    /// Tests whether this curve is a non-reversing line within the supplied
    /// modelling tolerance.
    ///
    /// This is a direct Rust translation of OpenNURBS' clamped-control-polygon
    /// predicate. Rational weights do not affect the result because positive
    /// weights preserve a collinear Euclidean control polygon.
    pub fn is_linear(&self, tolerance: Tolerance) -> Result<bool, GeometryError> {
        self.is_linear_with(LinearityTolerance::Absolute(tolerance.absolute()))
    }

    /// Tests linearity using OpenNURBS' zero-tolerance coordinate policy.
    /// Rhino's `SelLine` uses this stricter overload rather than document
    /// tolerance.
    pub fn is_linear_at_zero_tolerance(&self) -> Result<bool, GeometryError> {
        self.is_linear_with(LinearityTolerance::OpenNurbsZero)
    }

    /// Tests whether all Euclidean controls lie in the OpenNURBS candidate
    /// plane within the document's absolute tolerance.
    pub fn is_planar(&self, tolerance: Tolerance) -> Result<bool, GeometryError> {
        if self.is_linear(tolerance)? {
            return Ok(true);
        }
        let start = self.evaluate(*self.domain().start())?;
        let controls = self.control_points();
        control_polygon_is_planar_non_linear(
            controls.len(),
            |index| controls[index].point(),
            start,
            tolerance,
        )
    }

    fn is_linear_with(&self, tolerance: LinearityTolerance) -> Result<bool, GeometryError> {
        if !nurbs_is_clamped(self) {
            return Ok(false);
        }
        let controls = self.control_points();
        control_polygon_is_linear(controls.len(), |index| controls[index].point(), tolerance)
    }
}

#[derive(Clone, Copy)]
enum LinearityTolerance {
    OpenNurbsZero,
    Absolute(Real),
}

impl LinearityTolerance {
    fn distance(self) -> Real {
        match self {
            Self::OpenNurbsZero => CURVE_COINCIDENCE_ABSOLUTE,
            Self::Absolute(distance) => distance,
        }
    }

    fn points_coincide(self, left: Point3, right: Point3) -> Result<bool, GeometryError> {
        match self {
            Self::OpenNurbsZero => Ok(curve_points_coincident(left, right)),
            Self::Absolute(distance) => Ok(left.distance_to(right)? <= distance),
        }
    }
}

fn nurbs_is_clamped(curve: &NurbsCurve) -> bool {
    let knots = curve.knots();
    let degree = curve.degree();
    let control_count = curve.control_points().len();
    // OpenNURBS stores a knot vector without our two artificial end knots.
    // These are the equivalent exact start/end multiplicity comparisons.
    knots[1] == knots[degree] && knots[control_count] == knots[knots.len() - 2]
}

fn control_polygon_is_linear(
    point_count: usize,
    point_at: impl Fn(usize) -> Point3 + Copy,
    tolerance: LinearityTolerance,
) -> Result<bool, GeometryError> {
    debug_assert!(point_count >= 2);
    let start = point_at(0);
    let end = point_at(point_count - 1);
    let chord = start.vector_to(end)?;
    let chord_length = chord.length()?;
    if chord_length <= tolerance.distance() {
        return Ok(false);
    }
    if point_count == 2 {
        return Ok(true);
    }

    let direction = chord.normalized_nonzero()?;
    let mut previous_parameter = 0.0;
    for index in 1..point_count - 1 {
        let point = point_at(index);
        let from_start = start.vector_to(point)?;
        let from_end = end.vector_to(point)?;
        let parameter = if from_start.length()? <= from_end.length()? {
            from_start.dot(direction.as_vector())? / chord_length
        } else {
            1.0 + from_end.dot(direction.as_vector())? / chord_length
        };
        if !parameter.is_finite() || !(-0.01..=1.01).contains(&parameter) {
            return Ok(false);
        }

        let projected = start.translated(chord.scaled(parameter)?)?;
        if !tolerance.points_coincide(point, projected)? {
            return Ok(false);
        }

        if parameter > previous_parameter && previous_parameter < 1.0 {
            previous_parameter = parameter.min(1.0);
        }
        if !(parameter >= previous_parameter && parameter <= 1.0) {
            let previous = start.translated(chord.scaled(previous_parameter)?)?;
            if projected.distance_to(previous)? > tolerance.distance() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn control_polygon_is_planar(
    point_count: usize,
    point_at: impl Fn(usize) -> Point3 + Copy,
    start: Point3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if control_polygon_is_linear(
        point_count,
        point_at,
        LinearityTolerance::Absolute(tolerance.absolute()),
    )? {
        return Ok(true);
    }
    control_polygon_is_planar_non_linear(point_count, point_at, start, tolerance)
}

fn control_polygon_is_planar_non_linear(
    point_count: usize,
    point_at: impl Fn(usize) -> Point3 + Copy,
    start: Point3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if point_count < 3 {
        return Ok(false);
    }

    // This sampling stride deliberately matches OpenNURBS. All controls are
    // still checked against the resulting plane below.
    let stride = (point_count / 64).max(1);
    let mut largest_area = 0.0;
    let mut triangle = None;
    for first_index in (1..point_count).step_by(stride) {
        let first = point_at(first_index);
        for second_index in ((first_index + stride)..point_count).step_by(stride) {
            let second = point_at(second_index);
            let cross = start.vector_to(first)?.cross(start.vector_to(second)?)?;
            let area = cross.length()?;
            if area > largest_area {
                largest_area = area;
                triangle = Some(cross);
            }
        }
    }
    let Some(cross) = triangle else {
        return Ok(false);
    };
    let normal = cross.normalized_nonzero()?;
    for index in 0..point_count {
        let distance = start
            .vector_to(point_at(index))?
            .dot(normal.as_vector())?
            .abs();
        if distance > tolerance.absolute() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn require_division_capacity(point_count: usize) -> Result<(), GeometryError> {
    if point_count > MAX_CURVE_DIVISION_POINTS {
        Err(GeometryError::TooManyCurveDivisionPoints {
            maximum: MAX_CURVE_DIVISION_POINTS,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct ParameterSpan {
    start: Real,
    end: Real,
    length: Real,
    cumulative_start: Real,
    cumulative_end: Real,
    variable_speed: bool,
}

pub(crate) struct ArcLengthSampler<'a> {
    curve: CurveRef<'a>,
    spans: Vec<ParameterSpan>,
    lookup_tables: Vec<Vec<ArcLengthLookupNode>>,
    total_length: Real,
    tolerance: Tolerance,
}

#[derive(Clone, Copy, Debug)]
struct ArcLengthLookupNode {
    parameter: Real,
    length: Real,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ArcLengthKink {
    pub(crate) distance: Real,
    pub(crate) incoming_tangent: UnitVector3,
    pub(crate) outgoing_tangent: UnitVector3,
}

impl<'a> ArcLengthSampler<'a> {
    pub(crate) fn try_new(
        curve: CurveRef<'a>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let raw_spans = raw_spans(curve, tolerance)?;
        let mut spans = Vec::with_capacity(raw_spans.len());
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (start, end, length, variable_speed) in raw_spans {
            require_finite([start, end, length], "curve arc-length span")?;
            if start >= end || length < 0.0 {
                return Err(GeometryError::NumericalIntegrationDidNotConverge);
            }
            if length == 0.0 {
                continue;
            }
            let cumulative_start = sum + correction;
            neumaier_add(&mut sum, &mut correction, length);
            let cumulative_end = sum + correction;
            spans.push(ParameterSpan {
                start,
                end,
                length,
                cumulative_start,
                cumulative_end,
                variable_speed,
            });
        }
        let total_length = sum + correction;
        require_finite([total_length], "curve arc length")?;
        if spans.is_empty() || total_length <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "arc-length curve",
            });
        }
        Ok(Self {
            curve,
            lookup_tables: vec![Vec::new(); spans.len()],
            spans,
            total_length,
            tolerance,
        })
    }

    pub(crate) fn total_length(&self) -> Real {
        self.total_length
    }

    pub(crate) fn natural_break_distances(&self) -> impl Iterator<Item = Real> + '_ {
        self.spans
            .iter()
            .take(self.spans.len().saturating_sub(1))
            .map(|span| span.cumulative_end)
    }

    /// Precomputes exact-integration prefix brackets that make repeated
    /// arc-length inversions substantially cheaper. Ordinary one-shot curve
    /// queries avoid this setup cost; adaptive algorithms opt in explicitly.
    pub(crate) fn prepare_repeated_sampling(
        &mut self,
        subdivisions_per_span: usize,
    ) -> Result<(), GeometryError> {
        debug_assert!(subdivisions_per_span > 0);
        let mut tables = Vec::with_capacity(self.spans.len());
        for span in &self.spans {
            if !span.variable_speed {
                tables.push(Vec::new());
                continue;
            }
            let mut nodes = Vec::with_capacity(subdivisions_per_span + 1);
            nodes.push(ArcLengthLookupNode {
                parameter: span.start,
                length: 0.0,
            });
            let mut sum = 0.0;
            let mut correction = 0.0;
            let absolute_tolerance = numerical_distance_tolerance(span.length, self.tolerance)
                / subdivisions_per_span as Real;
            let mut previous = span.start;
            for division in 1..=subdivisions_per_span {
                let parameter = if division == subdivisions_per_span {
                    span.end
                } else {
                    stable_lerp(
                        span.start,
                        span.end,
                        division as Real / subdivisions_per_span as Real,
                    )
                };
                let length = integrate_adaptive(
                    previous,
                    parameter,
                    absolute_tolerance.max(Real::MIN_POSITIVE),
                    self.tolerance.relative(),
                    |value| self.speed(value),
                )?;
                neumaier_add(&mut sum, &mut correction, length);
                nodes.push(ArcLengthLookupNode {
                    parameter,
                    length: sum + correction,
                });
                previous = parameter;
            }
            let table_length = sum + correction;
            if !table_length.is_finite() || table_length <= 0.0 {
                return Err(GeometryError::NumericalIntegrationDidNotConverge);
            }
            tables.push(nodes);
        }
        self.lookup_tables = tables;
        Ok(())
    }

    /// Returns arc-length locations and one-sided tangents where adjacent
    /// natural spans meet at an angle larger than `angle_tolerance_radians`.
    pub(crate) fn kinks(
        &self,
        angle_tolerance_radians: Real,
    ) -> Result<Vec<ArcLengthKink>, GeometryError> {
        require_finite([angle_tolerance_radians], "curve kink angle tolerance")?;
        if !(0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians) {
            return Err(GeometryError::InvalidCurveFitAngleTolerance);
        }

        let minimum_dot = angle_tolerance_radians.cos();
        let roundoff = 128.0 * Real::EPSILON;
        let mut distances = Vec::new();
        for spans in self.spans.windows(2) {
            let left = spans[0];
            let right = spans[1];
            let left_parameter = left.end.next_down().max(left.start);
            let right_parameter = right.start.next_up().min(right.end);
            let left_tangent = self.curve.evaluate_with_tangent(left_parameter)?.tangent();
            let right_tangent = self.curve.evaluate_with_tangent(right_parameter)?.tangent();
            let dot = left_tangent
                .as_vector()
                .dot(right_tangent.as_vector())?
                .clamp(-1.0, 1.0);
            if dot < minimum_dot - roundoff {
                distances.push(ArcLengthKink {
                    distance: left.cumulative_end,
                    incoming_tangent: left_tangent,
                    outgoing_tangent: right_tangent,
                });
            }
        }
        Ok(distances)
    }

    fn parameter_start(&self) -> Real {
        self.spans[0].start
    }

    pub(crate) fn point_at_distance(&self, distance: Real) -> Result<Point3, GeometryError> {
        self.point_at_distance_impl(distance, None)
    }

    fn point_at_distance_with_fractional_tolerance(
        &self,
        distance: Real,
        fractional_tolerance: Real,
    ) -> Result<Point3, GeometryError> {
        self.point_at_distance_impl(distance, Some(fractional_tolerance))
    }

    fn point_at_distance_impl(
        &self,
        distance: Real,
        fractional_tolerance: Option<Real>,
    ) -> Result<Point3, GeometryError> {
        let parameter = self.parameter_at_distance_impl(distance, fractional_tolerance)?;
        if distance == self.total_length {
            self.curve.end_point()
        } else {
            self.evaluate(parameter)
        }
    }

    pub(crate) fn parameter_at_distance(&self, distance: Real) -> Result<Real, GeometryError> {
        self.parameter_at_distance_impl(distance, None)
    }

    fn parameter_at_distance_impl(
        &self,
        distance: Real,
        fractional_tolerance: Option<Real>,
    ) -> Result<Real, GeometryError> {
        require_finite([distance], "curve arc-length distance")?;
        if distance < 0.0 || distance > self.total_length {
            return Err(GeometryError::ArcLengthOutOfDomain {
                distance,
                length: self.total_length,
            });
        }
        if distance == 0.0 {
            return Ok(self.parameter_start());
        }
        if distance == self.total_length {
            return Ok(self.spans.last().expect("a sampler has spans").end);
        }

        let span_index = self
            .spans
            .partition_point(|span| span.cumulative_end < distance)
            .min(self.spans.len() - 1);
        let span = self.spans[span_index];
        let local_distance = (distance - span.cumulative_start).clamp(0.0, span.length);
        if local_distance == 0.0 {
            return Ok(span.start);
        }
        if local_distance == span.length {
            return Ok(span.end);
        }
        if !span.variable_speed {
            let fraction = local_distance / span.length;
            return Ok(stable_lerp(span.start, span.end, fraction));
        }

        let distance_tolerance = fractional_tolerance
            .map(|fractional| {
                (fractional * self.total_length.abs())
                    .max(64.0 * Real::EPSILON * self.total_length.abs())
                    .max(Real::MIN_POSITIVE)
            })
            .unwrap_or_else(|| numerical_distance_tolerance(span.length, self.tolerance));
        self.parameter_at_span_distance(span_index, span, local_distance, distance_tolerance)
    }

    pub(crate) fn sample_at_distance(&self, distance: Real) -> Result<CurveSample, GeometryError> {
        require_finite([distance], "curve arc-length distance")?;
        if distance < 0.0 || distance > self.total_length {
            return Err(GeometryError::ArcLengthOutOfDomain {
                distance,
                length: self.total_length,
            });
        }
        if distance == 0.0 {
            return self.curve.evaluate_with_tangent(self.parameter_start());
        }
        if distance == self.total_length {
            let parameter = self.spans.last().expect("a sampler has spans").end;
            let mut sample = self.curve.evaluate_with_tangent(parameter)?;
            sample.point = self.curve.end_point()?;
            return Ok(sample);
        }

        let span_index = self
            .spans
            .partition_point(|span| span.cumulative_end < distance)
            .min(self.spans.len() - 1);
        let span = self.spans[span_index];
        let local_distance = (distance - span.cumulative_start).clamp(0.0, span.length);
        if local_distance == 0.0 {
            return self.curve.evaluate_with_tangent(span.start);
        }
        if local_distance == span.length {
            return self.curve.evaluate_with_tangent(span.end);
        }
        if !span.variable_speed {
            let fraction = local_distance / span.length;
            return self
                .curve
                .evaluate_with_tangent(stable_lerp(span.start, span.end, fraction));
        }

        let distance_tolerance = numerical_distance_tolerance(span.length, self.tolerance);
        let parameter =
            self.parameter_at_span_distance(span_index, span, local_distance, distance_tolerance)?;
        self.curve.evaluate_with_tangent(parameter)
    }

    fn parameter_at_span_distance(
        &self,
        span_index: usize,
        span: ParameterSpan,
        target: Real,
        distance_tolerance: Real,
    ) -> Result<Real, GeometryError> {
        let table = &self.lookup_tables[span_index];
        let inversion_target = if table.is_empty() {
            target
        } else {
            let table_length = table.last().expect("a lookup table has an end").length;
            target * (table_length / span.length)
        };
        let (prefix_parameter, prefix_length, mut lower, mut upper, mut parameter) =
            if table.is_empty() {
                (
                    span.start,
                    0.0,
                    span.start,
                    span.end,
                    stable_lerp(span.start, span.end, target / span.length),
                )
            } else {
                let upper_index = table
                    .partition_point(|node| node.length < inversion_target)
                    .clamp(1, table.len() - 1);
                let lower_node = table[upper_index - 1];
                let upper_node = table[upper_index];
                if inversion_target == lower_node.length {
                    return Ok(lower_node.parameter);
                }
                if inversion_target == upper_node.length {
                    return Ok(upper_node.parameter);
                }
                let fraction = (inversion_target - lower_node.length)
                    / (upper_node.length - lower_node.length);
                (
                    lower_node.parameter,
                    lower_node.length,
                    lower_node.parameter,
                    upper_node.parameter,
                    stable_lerp(lower_node.parameter, upper_node.parameter, fraction),
                )
            };

        for _ in 0..80 {
            let length = prefix_length
                + self.partial_parameter_length(prefix_parameter, parameter, distance_tolerance)?;
            let residual = length - inversion_target;
            if residual.abs() <= distance_tolerance {
                return Ok(parameter);
            }
            if residual < 0.0 {
                lower = parameter;
            } else {
                upper = parameter;
            }

            let midpoint = lower * 0.5 + upper * 0.5;
            if midpoint <= lower || midpoint >= upper {
                return Ok(midpoint.clamp(span.start, span.end));
            }
            let speed = self.speed(parameter)?;
            let newton = (speed > 0.0)
                .then(|| parameter - residual / speed)
                .filter(|candidate| {
                    candidate.is_finite() && *candidate > lower && *candidate < upper
                });
            parameter = newton.unwrap_or(midpoint);
        }
        Err(GeometryError::NumericalIntegrationDidNotConverge)
    }

    fn partial_parameter_length(
        &self,
        start: Real,
        parameter: Real,
        absolute_tolerance: Real,
    ) -> Result<Real, GeometryError> {
        if parameter <= start {
            return Ok(0.0);
        }
        integrate_adaptive(
            start,
            parameter,
            absolute_tolerance,
            self.tolerance.relative(),
            |value| self.speed(value),
        )
    }

    fn speed(&self, parameter: Real) -> Result<Real, GeometryError> {
        let speed = match self.curve {
            CurveRef::Line(line) => line.length()?,
            CurveRef::Circle(circle) => circle.radius(),
            CurveRef::Arc(arc) => arc.length()?,
            CurveRef::Ellipse(ellipse) => {
                let (sine, cosine) = parameter.sin_cos();
                (ellipse.radius_x() * sine).hypot(ellipse.radius_y() * cosine)
            }
            CurveRef::Polyline(polyline) => {
                let index = (parameter.floor() as usize).min(polyline.segment_count() - 1);
                polyline.vertices()[index].distance_to(polyline.vertices()[index + 1])?
            }
            CurveRef::NurbsCurve(curve) => curve.derivative_at(parameter)?.length()?,
            CurveRef::PolyCurve(curve) => curve.evaluate_with_derivative(parameter)?.1.length()?,
        };
        require_finite([speed], "curve parameter speed")?;
        Ok(speed)
    }

    fn evaluate(&self, parameter: Real) -> Result<Point3, GeometryError> {
        match self.curve {
            CurveRef::Line(line) => line.point_at(parameter),
            CurveRef::Circle(circle) => circle.point_at_angle(parameter),
            CurveRef::Arc(arc) => arc.point_at(parameter),
            CurveRef::Ellipse(ellipse) => ellipse.point_at_angle(parameter),
            CurveRef::Polyline(polyline) => {
                if parameter >= polyline.segment_count() as Real {
                    return Ok(*polyline.vertices().last().expect("a polyline has vertices"));
                }
                let index = (parameter.floor() as usize).min(polyline.segment_count() - 1);
                let fraction = parameter - index as Real;
                LineSegment::from_validated(
                    polyline.vertices()[index],
                    polyline.vertices()[index + 1],
                )
                .point_at(fraction)
            }
            CurveRef::NurbsCurve(curve) => curve.evaluate(parameter),
            CurveRef::PolyCurve(curve) => curve.evaluate(parameter),
        }
    }
}

fn raw_spans(
    curve: CurveRef<'_>,
    tolerance: Tolerance,
) -> Result<Vec<(Real, Real, Real, bool)>, GeometryError> {
    Ok(match curve {
        CurveRef::Line(line) => vec![(0.0, 1.0, line.length()?, false)],
        CurveRef::Circle(circle) => {
            let quadrant_length = circle.length()? * 0.25;
            (0..4)
                .map(|quadrant| {
                    let start = quadrant as Real * FRAC_PI_2;
                    (start, start + FRAC_PI_2, quadrant_length, false)
                })
                .collect()
        }
        CurveRef::Arc(arc) => vec![(0.0, 1.0, arc.length()?, false)],
        CurveRef::Ellipse(ellipse) => {
            let quadrant_length = integrate_speed(0.0, FRAC_PI_2, tolerance, |angle| {
                let (sine, cosine) = angle.sin_cos();
                let speed = (ellipse.radius_x() * sine).hypot(ellipse.radius_y() * cosine);
                require_finite([speed], "ellipse speed")?;
                Ok(speed)
            })?;
            (0..4)
                .map(|quadrant| {
                    let start = quadrant as Real * FRAC_PI_2;
                    (start, start + FRAC_PI_2, quadrant_length, true)
                })
                .collect()
        }
        CurveRef::Polyline(polyline) => polyline
            .segments()
            .enumerate()
            .map(|(index, segment)| {
                Ok((index as Real, (index + 1) as Real, segment.length()?, false))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        CurveRef::NurbsCurve(curve) => curve
            .spans()
            .map(|(start, end)| {
                let length = integrate_speed(start, end, tolerance, |parameter| {
                    curve.derivative_at(parameter)?.length()
                })?;
                Ok((start, end, length, true))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        CurveRef::PolyCurve(curve) => {
            let mut spans = Vec::new();
            for (index, segment) in curve.segments().iter().enumerate() {
                for (start, end) in segment.spans() {
                    let length = integrate_speed(start, end, tolerance, |parameter| {
                        segment.derivative_at(parameter)?.length()
                    })?;
                    spans.push((
                        curve.polycurve_parameter(index, start)?,
                        curve.polycurve_parameter(index, end)?,
                        length,
                        true,
                    ));
                }
            }
            spans
        }
    })
}

fn integrate_speed(
    start: Real,
    end: Real,
    tolerance: Tolerance,
    mut speed: impl FnMut(Real) -> Result<Real, GeometryError>,
) -> Result<Real, GeometryError> {
    let coarse = integrate_adaptive(
        start,
        end,
        tolerance.absolute(),
        tolerance.relative(),
        &mut speed,
    )?;
    let tighter = numerical_distance_tolerance(coarse, tolerance);
    if tighter < tolerance.absolute() {
        integrate_adaptive(start, end, tighter, tolerance.relative(), speed)
    } else {
        Ok(coarse)
    }
}

fn numerical_distance_tolerance(length: Real, tolerance: Tolerance) -> Real {
    let relative = tolerance.relative() * length.abs();
    let roundoff = 64.0 * Real::EPSILON * length.abs();
    tolerance
        .absolute()
        .min(relative)
        .max(roundoff)
        .max(Real::MIN_POSITIVE)
}

fn stable_lerp(start: Real, end: Real, fraction: Real) -> Real {
    start.mul_add(1.0 - fraction, end * fraction)
}

fn neumaier_add(sum: &mut Real, correction: &mut Real, value: Real) {
    let next = *sum + value;
    if sum.abs() >= value.abs() {
        *correction += (*sum - next) + value;
    } else {
        *correction += (value - next) + *sum;
    }
    *sum = next;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UnitVector3, WeightedPoint3};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn axis(x: Real, y: Real, z: Real) -> UnitVector3 {
        UnitVector3::try_new(x, y, z, Tolerance::DEFAULT).unwrap()
    }

    fn clamped_curve(degree: usize, points: Vec<Point3>) -> NurbsCurve {
        NurbsCurve::try_clamped_uniform(degree, points).unwrap()
    }

    #[test]
    fn classifies_clamped_nurbs_linearity_with_opennurbs_rules() {
        let cubic = clamped_curve(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
            ],
        );
        assert!(cubic.is_linear_at_zero_tolerance().unwrap());

        let near = clamped_curve(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 5.0e-10, 0.0),
                point(2.0, 5.0e-10, 0.0),
                point(3.0, 0.0, 0.0),
            ],
        );
        assert!(!near.is_linear_at_zero_tolerance().unwrap());
        assert!(near.is_linear(Tolerance::DEFAULT).unwrap());

        let reversing = clamped_curve(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
            ],
        );
        assert!(!reversing.is_linear(Tolerance::DEFAULT).unwrap());

        let unclamped = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        )
        .unwrap();
        assert!(!unclamped.is_linear(Tolerance::DEFAULT).unwrap());
    }

    #[test]
    fn classifies_analytic_polyline_and_rational_nurbs_planarity() {
        let planar = NurbsCurve::try_new_rational(
            3,
            vec![
                WeightedPoint3::try_new(point(0.0, 0.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 0.0, 1.0), 0.5).unwrap(),
                WeightedPoint3::try_new(point(1.0, 2.0, 3.0), 2.0).unwrap(),
                WeightedPoint3::try_new(point(3.0, -1.0, 2.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(
            CurveRef::NurbsCurve(&planar)
                .is_planar(Tolerance::DEFAULT)
                .unwrap()
        );

        let nonplanar = clamped_curve(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 2.0),
            ],
        );
        assert!(
            !CurveRef::NurbsCurve(&nonplanar)
                .is_planar(Tolerance::DEFAULT)
                .unwrap()
        );

        let bent_polyline = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 2.0, 1.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            !CurveRef::Polyline(&bent_polyline)
                .is_planar(Tolerance::DEFAULT)
                .unwrap()
        );

        let line = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(1.0, 2.0, 3.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(CurveRef::Line(&line).is_planar(Tolerance::DEFAULT).unwrap());
    }

    #[test]
    fn divides_lines_and_polylines_at_exact_arc_lengths() {
        let line = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let points = CurveRef::Line(&line)
            .divide_by_count(5, false, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(
            points,
            (1..5)
                .map(|index| point(index as Real * 2.0, 0.0, 0.0))
                .collect::<Vec<_>>()
        );

        let polyline = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(3.0, 4.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let points = CurveRef::Polyline(&polyline)
            .divide_by_count(7, true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(points[0], point(0.0, 0.0, 0.0));
        assert_eq!(points[3], point(3.0, 0.0, 0.0));
        assert_eq!(points[7], point(3.0, 4.0, 0.0));
        for points in points.windows(2) {
            assert_eq!(points[0].distance_to(points[1]).unwrap(), 1.0);
        }

        let huge_end = Real::MAX * 0.5;
        let huge = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(huge_end, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let points = CurveRef::Line(&huge)
            .divide_by_count(5, true, Tolerance::DEFAULT)
            .unwrap();
        assert!(points.iter().all(|point| point.x().is_finite()));
        assert_eq!(points[5], huge.end());
    }

    #[test]
    fn divides_closed_analytic_curves_without_repeating_the_seam() {
        let circle = Circle3::try_new(
            point(1.0, 2.0, 3.0),
            2.0,
            axis(0.0, 0.0, 1.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let points = CurveRef::Circle(&circle)
            .divide_by_count(4, true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(points.len(), 4);
        assert_eq!(points[0], circle.quadrants().unwrap()[0]);
        for (actual, expected) in points[..4].iter().zip(circle.quadrants().unwrap()) {
            assert!(actual.is_near(expected, Tolerance::DEFAULT));
        }

        let ellipse = Ellipse3::try_new(
            point(0.0, 0.0, 0.0),
            5.0,
            2.0,
            axis(1.0, 0.0, 0.0),
            axis(0.0, 1.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let points = CurveRef::Ellipse(&ellipse)
            .divide_by_count(8, true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(points.len(), 8);
        for quadrant in 0..4 {
            assert!(points[quadrant * 2].is_near(
                ellipse.quadrants().unwrap()[quadrant],
                Tolerance::try_new(1.0e-11, 1.0e-12, 1.0e-12).unwrap()
            ));
        }
        let samples = CurveRef::Circle(&circle)
            .sample_equal_length_points(4, true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(samples.len(), 5);
        assert_eq!(samples[0], samples[4]);
        let without_start = CurveRef::Circle(&circle)
            .divide_by_count(4, false, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(without_start.len(), 3);
        assert_eq!(without_start, samples[1..4]);
    }

    #[test]
    fn samples_equal_arc_lengths_with_natural_parameters_and_unit_tangents() {
        let circle = Circle3::try_from_center_point(
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            axis(0.0, 0.0, 1.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let samples = CurveRef::Circle(&circle)
            .divide_by_count_samples(4, true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(samples.len(), 5);
        let expected_tangents = [
            axis(0.0, 1.0, 0.0),
            axis(-1.0, 0.0, 0.0),
            axis(0.0, -1.0, 0.0),
            axis(1.0, 0.0, 0.0),
            axis(0.0, 1.0, 0.0),
        ];
        for ((sample, expected_point), expected_tangent) in samples
            .iter()
            .zip([
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(-2.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
                point(2.0, 0.0, 0.0),
            ])
            .zip(expected_tangents)
        {
            assert!(sample.point().is_near(expected_point, Tolerance::DEFAULT));
            for (actual, expected) in sample
                .tangent()
                .as_vector()
                .to_array()
                .into_iter()
                .zip(expected_tangent.as_vector().to_array())
            {
                assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
            }
        }
        assert_eq!(samples[0].parameter(), 0.0);
        assert_eq!(samples[4].point(), samples[0].point());
        assert_eq!(
            samples[1].reversed_direction().tangent(),
            samples[1].tangent().opposite()
        );

        let line = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let samples = CurveRef::Line(&line)
            .divide_by_length_samples(3.0, true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.point())
                .collect::<Vec<_>>(),
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(6.0, 0.0, 0.0),
                point(9.0, 0.0, 0.0),
            ]
        );
        assert!(
            samples
                .iter()
                .all(|sample| sample.tangent() == axis(1.0, 0.0, 0.0))
        );
        assert!(matches!(
            CurveRef::Circle(&circle).evaluate_with_tangent(-1.0),
            Err(GeometryError::ParameterOutOfDomain { .. })
        ));

        // Point-only division remains valid at a stationary curve endpoint;
        // only the tangent-bearing API requires a regular sample there.
        let stationary_start = clamped_curve(
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
            ],
        );
        assert_eq!(
            CurveRef::NurbsCurve(&stationary_start)
                .divide_by_count(1, true, Tolerance::DEFAULT)
                .unwrap(),
            vec![point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0)]
        );
        assert!(matches!(
            CurveRef::NurbsCurve(&stationary_start).start_sample(Tolerance::DEFAULT),
            Err(GeometryError::Degenerate { .. })
        ));

        let slowly_parameterized = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0)],
            vec![0.0, 0.0, 1.0e20, 1.0e20],
        )
        .unwrap();
        let sample = CurveRef::NurbsCurve(&slowly_parameterized)
            .evaluate_with_tangent(5.0e19)
            .unwrap();
        assert!(
            sample
                .point()
                .is_near(point(0.5, 0.0, 0.0), Tolerance::DEFAULT)
        );
        assert_eq!(sample.tangent(), axis(1.0, 0.0, 0.0));
    }

    #[test]
    fn inverts_rational_nurbs_arc_length_to_circle_accuracy() {
        let weight = std::f64::consts::FRAC_1_SQRT_2;
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(1.0, 0.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0, 0.0), weight).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0, 0.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let points = CurveRef::NurbsCurve(&curve)
            .divide_by_count(8, true, Tolerance::DEFAULT)
            .unwrap();
        let tolerance = Tolerance::try_new(2.0e-12, 2.0e-12, 1.0e-12).unwrap();
        for (index, actual) in points.iter().enumerate() {
            let angle = FRAC_PI_2 * index as Real / 8.0;
            assert!(actual.is_near(point(angle.cos(), angle.sin(), 0.0), tolerance));
        }
    }

    #[test]
    fn divides_by_length_and_rejects_unbounded_requests() {
        let line = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            CurveRef::Line(&line)
                .divide_by_length(3.0, true, Tolerance::DEFAULT)
                .unwrap(),
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(6.0, 0.0, 0.0),
                point(9.0, 0.0, 0.0),
            ]
        );
        assert!(matches!(
            CurveRef::Line(&line).divide_by_count(
                MAX_CURVE_DIVISION_POINTS,
                true,
                Tolerance::DEFAULT
            ),
            Err(GeometryError::TooManyCurveDivisionPoints { .. })
        ));
        assert_eq!(
            CurveRef::Line(&line).divide_by_length(0.0, false, Tolerance::DEFAULT),
            Err(GeometryError::InvalidCurveDivisionLength)
        );
    }
}
