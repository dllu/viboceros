#[cfg(test)]
use std::f64::consts::FRAC_PI_2;

mod arc_length;
pub(crate) use arc_length::{ArcLengthKink, ArcLengthSampler};

use crate::{
    Circle3, CircularArc3, Ellipse3, GeometryError, LineSegment, NurbsCurve, ParameterSide, Point3,
    PolyCurve3, Polyline3, Real, Tolerance, UnitVector3, Vector3,
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
            Self::Line(_) => false,
            Self::Arc(arc) => arc.is_closed(),
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
                let mut controls = Vec::new();
                for segment in curve.segments() {
                    controls.extend(
                        segment
                            .to_nurbs()?
                            .control_points()
                            .iter()
                            .map(|c| c.point()),
                    );
                }
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
                sampler.total_length()
            } else {
                sampler.total_length() * (index as Real / segment_count as Real)
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
        let quotient = (sampler.total_length() / segment_length).floor();
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
            if distance > sampler.total_length() {
                if tolerance.approx_eq(distance, sampler.total_length()) {
                    distance = sampler.total_length();
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
                sampler.total_length()
            } else {
                sampler.total_length() * (index as Real / segment_count as Real)
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
        let quotient = (sampler.total_length() / segment_length).floor();
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
            if distance > sampler.total_length() {
                if tolerance.approx_eq(distance, sampler.total_length()) {
                    distance = sampler.total_length();
                } else {
                    break;
                }
            }
            samples.push(sampler.sample_at_distance(distance)?);
        }
        Ok(samples)
    }

    /// Evaluates the native parameter and its oriented unit tangent.
    pub fn evaluate_with_tangent(self, parameter: Real) -> Result<CurveSample, GeometryError> {
        self.evaluate_with_tangent_on_side(parameter, ParameterSide::Right)
    }

    /// One-sided oriented tangent at the exact parameter, without a parameter
    /// perturbation. NURBS stationary points use the first nonzero higher
    /// derivative; locally constant spans return a degeneracy error.
    pub fn evaluate_with_tangent_on_side(
        self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<CurveSample, GeometryError> {
        let point = self.evaluate_on_side(parameter, side)?;
        // Direction is independent of domain width. Avoid overflowing or
        // underflowing a derivative solely to normalize it immediately.
        let derivative = match self {
            Self::Line(c) => c.start().vector_to(c.end())?,
            Self::Circle(c) => angular_tangent(
                c.x_axis(),
                c.y_axis(),
                crate::parameter::map_parameter(
                    parameter,
                    c.domain(),
                    0.0..=std::f64::consts::TAU,
                )?,
            )?,
            Self::Arc(c) => angular_tangent(
                c.x_axis(),
                c.y_axis(),
                crate::parameter::map_parameter(parameter, c.domain(), 0.0..=c.sweep_radians())?,
            )?,
            Self::Ellipse(c) => {
                let jet =
                    crate::curve_evaluate::ellipse_unit_jet_on_side(parameter, c.domain(), side)?;
                combine_vectors(
                    c.x_axis().as_vector(),
                    c.y_axis().as_vector(),
                    c.radius_x() * jet[1][0],
                    c.radius_y() * jet[1][1],
                )?
            }
            Self::Polyline(c) => {
                let (i, _) = c.parameter_location_on_side(parameter, side)?;
                c.vertices()[i].vector_to(c.vertices()[i + 1])?
            }
            Self::NurbsCurve(c) => {
                return Ok(CurveSample {
                    parameter,
                    point,
                    tangent: c.tangent_at_on_side(parameter, side)?,
                });
            }
            Self::PolyCurve(c) => {
                let index = c.segment_index(parameter, side)?;
                c.segments()[index]
                    .as_ref()
                    .evaluate_with_tangent_on_side(c.segment_parameter(index, parameter)?, side)?
                    .tangent()
                    .as_vector()
            }
        };
        Ok(CurveSample {
            parameter,
            point,
            tangent: derivative.normalized_nonzero()?,
        })
    }

    /// Returns the derivative of the unit tangent with respect to arc length.
    ///
    /// This is the curvature vector, including its model-space direction. At
    /// the interior of a polyline segment it is zero; vertices have no unique
    /// curvature and deterministically use the active segment's zero value.
    pub fn curvature_vector(self, parameter: Real) -> Result<Vector3, GeometryError> {
        self.evaluate(parameter)?;
        match self {
            Self::Line(_) | Self::Polyline(_) => Vector3::try_new(0.0, 0.0, 0.0),
            Self::Circle(circle) => radial_curvature(
                circle.x_axis(),
                circle.y_axis(),
                circle.radius(),
                crate::parameter::map_parameter(
                    parameter,
                    circle.domain(),
                    0.0..=std::f64::consts::TAU,
                )?,
            ),
            Self::Arc(arc) => radial_curvature(
                arc.x_axis(),
                arc.y_axis(),
                arc.radius(),
                crate::parameter::map_parameter(
                    parameter,
                    arc.domain(),
                    0.0..=arc.sweep_radians(),
                )?,
            ),
            Self::Ellipse(ellipse) => {
                let jet = crate::curve_evaluate::ellipse_unit_jet(parameter, ellipse.domain())?;
                let first = combine_vectors(
                    ellipse.x_axis().as_vector(),
                    ellipse.y_axis().as_vector(),
                    ellipse.radius_x() * jet[1][0],
                    ellipse.radius_y() * jet[1][1],
                )?;
                let second = combine_vectors(
                    ellipse.x_axis().as_vector(),
                    ellipse.y_axis().as_vector(),
                    ellipse.radius_x() * jet[2][0],
                    ellipse.radius_y() * jet[2][1],
                )?;
                curvature_from_derivatives(first, second)
            }
            Self::NurbsCurve(curve) => {
                let (_, first, second) = curve.evaluate_with_second_derivative(parameter)?;
                curvature_from_derivatives(first, second)
            }
            Self::PolyCurve(curve) => {
                let (_, first, second) =
                    curve.evaluate_with_second_derivative(parameter, ParameterSide::Right)?;
                curvature_from_derivatives(first, second)
            }
        }
    }
}

fn angular_tangent(x: UnitVector3, y: UnitVector3, angle: Real) -> Result<Vector3, GeometryError> {
    let (sine, cosine) = angle.sin_cos();
    combine_vectors(x.as_vector(), y.as_vector(), -sine, cosine)
}

fn radial_curvature(
    x: UnitVector3,
    y: UnitVector3,
    radius: Real,
    angle: Real,
) -> Result<Vector3, GeometryError> {
    let (sine, cosine) = angle.sin_cos();
    crate::curve_evaluate::scale(
        combine_vectors(x.as_vector(), y.as_vector(), -cosine, -sine)?,
        1.0,
        radius,
    )
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

        // C(t)=2t^2 has zero first derivative at the start, but its oriented
        // limiting tangent exists and is supplied by the second derivative.
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
        assert_eq!(
            CurveRef::NurbsCurve(&stationary_start)
                .start_sample(Tolerance::DEFAULT)
                .unwrap()
                .tangent(),
            axis(1.0, 0.0, 0.0)
        );

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
