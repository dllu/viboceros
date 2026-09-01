use std::f64::consts::FRAC_PI_2;

use crate::{
    Circle3, CircularArc3, Ellipse3, GeometryError, LineSegment, NurbsCurve, Point3, Polyline3,
    Real, Tolerance, UnitVector3, Vector3,
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
        }
    }

    /// Returns whether the curve has coincident natural endpoints.
    pub fn is_closed(self) -> Result<bool, GeometryError> {
        Ok(match self {
            Self::Circle(_) | Self::Ellipse(_) => true,
            Self::Line(_) | Self::Arc(_) => false,
            Self::Polyline(polyline) => polyline.is_closed(),
            Self::NurbsCurve(curve) => curve.is_closed()?,
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
        }
    }

    /// Divides the curve into `segment_count` equal arc-length segments.
    ///
    /// The natural end is always returned. When `include_start` is true, the
    /// natural start is returned as well. This mirrors RhinoCommon's
    /// `Curve.DivideByCount` contract; a closed curve therefore returns its
    /// seam twice when both ends are requested.
    pub fn divide_by_count(
        self,
        segment_count: usize,
        include_start: bool,
        tolerance: Tolerance,
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
            points.push(sampler.point_at_distance(distance)?);
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
        };
        Ok(CurveSample {
            parameter,
            point,
            // A derivative's magnitude depends on parameter scaling, so model
            // distance tolerance must not decide whether its direction exists.
            tangent: derivative.normalized_nonzero()?,
        })
    }
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

struct ArcLengthSampler<'a> {
    curve: CurveRef<'a>,
    spans: Vec<ParameterSpan>,
    total_length: Real,
    tolerance: Tolerance,
}

impl<'a> ArcLengthSampler<'a> {
    fn try_new(curve: CurveRef<'a>, tolerance: Tolerance) -> Result<Self, GeometryError> {
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
            spans,
            total_length,
            tolerance,
        })
    }

    fn parameter_start(&self) -> Real {
        self.spans[0].start
    }

    fn point_at_distance(&self, distance: Real) -> Result<Point3, GeometryError> {
        require_finite([distance], "curve arc-length distance")?;
        if distance < 0.0 || distance > self.total_length {
            return Err(GeometryError::ArcLengthOutOfDomain {
                distance,
                length: self.total_length,
            });
        }
        if distance == 0.0 {
            return self.evaluate(self.parameter_start());
        }
        if distance == self.total_length {
            return self.curve.end_point();
        }

        let span_index = self
            .spans
            .partition_point(|span| span.cumulative_end < distance)
            .min(self.spans.len() - 1);
        let span = self.spans[span_index];
        let local_distance = (distance - span.cumulative_start).clamp(0.0, span.length);
        if local_distance == 0.0 {
            return self.evaluate(span.start);
        }
        if local_distance == span.length {
            return self.evaluate(span.end);
        }
        if !span.variable_speed {
            let fraction = local_distance / span.length;
            return self.evaluate(stable_lerp(span.start, span.end, fraction));
        }

        let parameter = self.parameter_at_span_distance(span, local_distance)?;
        self.evaluate(parameter)
    }

    fn sample_at_distance(&self, distance: Real) -> Result<CurveSample, GeometryError> {
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

        let parameter = self.parameter_at_span_distance(span, local_distance)?;
        self.curve.evaluate_with_tangent(parameter)
    }

    fn parameter_at_span_distance(
        &self,
        span: ParameterSpan,
        target: Real,
    ) -> Result<Real, GeometryError> {
        let distance_tolerance = numerical_distance_tolerance(span.length, self.tolerance);
        let mut lower = span.start;
        let mut upper = span.end;
        let mut parameter = stable_lerp(span.start, span.end, target / span.length);

        for _ in 0..80 {
            let length = self.partial_span_length(span, parameter, distance_tolerance)?;
            let residual = length - target;
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

    fn partial_span_length(
        &self,
        span: ParameterSpan,
        parameter: Real,
        absolute_tolerance: Real,
    ) -> Result<Real, GeometryError> {
        if parameter <= span.start {
            return Ok(0.0);
        }
        if parameter >= span.end {
            return Ok(span.length);
        }
        integrate_adaptive(
            span.start,
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
            (1..=5)
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
    fn divides_closed_analytic_curves_without_losing_the_exact_seam() {
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
        assert_eq!(points.len(), 5);
        assert_eq!(points[0], points[4]);
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
        assert_eq!(points[0], points[8]);
        for quadrant in 0..4 {
            assert!(points[quadrant * 2].is_near(
                ellipse.quadrants().unwrap()[quadrant],
                Tolerance::try_new(1.0e-11, 1.0e-12, 1.0e-12).unwrap()
            ));
        }
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
