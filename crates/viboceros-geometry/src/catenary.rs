use crate::{
    CurveInterpolationOptions, CurveKnotSpacing, GeometryError, InterpolatedCurveClosure,
    MAX_CURVE_INTERPOLATION_POINTS, NurbsCurve, Point3, Polyline3, Real, Tolerance, UnitVector3,
    Vector3, require_finite,
};

const ROOT_ITERATIONS: usize = 160;
const CUBIC_DEGREE: usize = 3;

/// Default number of output control points used by Rhino's catenary helpers.
pub const DEFAULT_CATENARY_POINT_COUNT: usize = 20;

/// Smallest point count that can produce a cubic with both end tangents fixed.
pub const MIN_SMOOTH_CATENARY_POINT_COUNT: usize = 4;

/// Smallest useful catenary polyline point count.
pub const MIN_POLYLINE_CATENARY_POINT_COUNT: usize = 2;

/// Resource ceiling for one catenary approximation.
pub const MAX_CATENARY_POINT_COUNT: usize = MAX_CURVE_INTERPOLATION_POINTS + 2;

/// Constraint used to determine a catenary's analytic parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CatenaryConstruction {
    /// Constrains an interior location on the analytic catenary.
    ThroughPoint(Point3),
    /// Constrains the analytic arc length between the endpoints.
    Length(Real),
    /// Supplies `a` in the equation `y = a cosh(x / a)`.
    Parameter(Real),
    /// Constrains the apex height along the gravity axis.
    Apex(Point3),
}

/// Representation requested for a catenary approximation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatenaryOutput {
    /// A clamped cubic NURBS approximation with exact analytic end tangents.
    Smooth,
    /// A piecewise-linear approximation sampled at uniform horizontal stations.
    Polyline,
}

impl CatenaryOutput {
    const fn minimum_point_count(self) -> usize {
        match self {
            Self::Smooth => MIN_SMOOTH_CATENARY_POINT_COUNT,
            Self::Polyline => MIN_POLYLINE_CATENARY_POINT_COUNT,
        }
    }
}

/// Geometry produced by a catenary construction.
#[derive(Clone, Debug, PartialEq)]
pub enum CatenaryCurve {
    Smooth(NurbsCurve),
    Polyline(Polyline3),
}

impl CatenaryCurve {
    /// Converts either representation to its exact NURBS encoding.
    pub fn to_nurbs(&self) -> Result<NurbsCurve, GeometryError> {
        match self {
            Self::Smooth(curve) => Ok(curve.clone()),
            Self::Polyline(polyline) => polyline.to_nurbs(),
        }
    }
}

/// A catenary approximation and the solved analytic quantities behind it.
#[derive(Clone, Debug, PartialEq)]
pub struct CatenarySolution {
    curve: CatenaryCurve,
    apex: Point3,
    parameter: Real,
    length: Real,
}

impl CatenarySolution {
    #[inline]
    pub const fn curve(&self) -> &CatenaryCurve {
        &self.curve
    }

    #[inline]
    pub const fn apex(&self) -> Point3 {
        self.apex
    }

    #[inline]
    pub const fn parameter(&self) -> Real {
        self.parameter
    }

    #[inline]
    pub const fn length(&self) -> Real {
        self.length
    }

    #[inline]
    pub fn into_curve(self) -> CatenaryCurve {
        self.curve
    }
}

/// Constructs the catenary lying in the plane spanned by its endpoint chord
/// and `axis_direction`.
///
/// The axis points toward the sagging side. Smooth output matches Rhino's
/// approximation policy: `point_count - 2` analytic points are sampled at
/// uniform horizontal stations, chord-parameterized, and interpolated by a
/// cubic whose end tangent directions come from the exact catenary. Polyline
/// output uses `point_count` uniform horizontal stations directly.
pub fn try_catenary(
    start: Point3,
    end: Point3,
    axis_direction: Vector3,
    construction: CatenaryConstruction,
    output: CatenaryOutput,
    point_count: usize,
    tolerance: Tolerance,
) -> Result<CatenarySolution, GeometryError> {
    let minimum = output.minimum_point_count();
    if !(minimum..=MAX_CATENARY_POINT_COUNT).contains(&point_count) {
        return Err(GeometryError::InvalidCatenaryPointCount {
            actual: point_count,
            minimum,
            maximum: MAX_CATENARY_POINT_COUNT,
        });
    }

    let frame = CatenaryFrame::try_new(start, end, axis_direction, tolerance)?;
    let parameter = match construction {
        CatenaryConstruction::Parameter(parameter) => {
            require_finite([parameter], "catenary parameter")?;
            if parameter <= 0.0 {
                return Err(GeometryError::InvalidCatenaryParameter);
            }
            parameter
        }
        CatenaryConstruction::Length(length) => frame.parameter_from_length(length)?,
        CatenaryConstruction::Apex(apex) => frame.parameter_from_apex(apex)?,
        CatenaryConstruction::ThroughPoint(point) => {
            frame.parameter_through_point(point, tolerance)?
        }
    };
    let analytic = AnalyticCatenary::try_new(frame, parameter)?;
    let curve = match output {
        CatenaryOutput::Polyline => {
            let points = analytic.uniform_points(point_count)?;
            CatenaryCurve::Polyline(Polyline3::try_new(points, tolerance)?)
        }
        CatenaryOutput::Smooth => {
            let points = analytic.uniform_points(point_count - 2)?;
            let options = CurveInterpolationOptions::new(
                CUBIC_DEGREE,
                CurveKnotSpacing::Chord,
                InterpolatedCurveClosure::Open,
            )
            .with_start_tangent(analytic.tangent_at_horizontal(0.0)?)
            .with_end_tangent(analytic.tangent_at_horizontal(analytic.frame.horizontal_span)?);
            CatenaryCurve::Smooth(NurbsCurve::try_interpolate(&points, options, tolerance)?)
        }
    };

    Ok(CatenarySolution {
        curve,
        apex: analytic.apex()?,
        parameter,
        length: analytic.length,
    })
}

#[derive(Clone, Copy, Debug)]
struct CatenaryFrame {
    start: Point3,
    horizontal_axis: UnitVector3,
    gravity_axis: UnitVector3,
    horizontal_span: Real,
    end_height: Real,
}

impl CatenaryFrame {
    fn try_new(
        start: Point3,
        end: Point3,
        axis_direction: Vector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let gravity_axis = axis_direction
            .normalized(tolerance)
            .map_err(|error| match error {
                GeometryError::Degenerate { .. } => GeometryError::Degenerate {
                    context: "catenary axis",
                },
                error => error,
            })?;
        let chord = start.vector_to(end)?;
        let end_height = chord.dot(gravity_axis.as_vector())?;
        let axial = gravity_axis.as_vector().scaled(end_height)?;
        let horizontal = subtract_vectors(chord, axial)?;
        let horizontal_axis = horizontal
            .normalized(tolerance)
            .map_err(|error| match error {
                GeometryError::Degenerate { .. } => GeometryError::Degenerate {
                    context: "catenary endpoint span",
                },
                error => error,
            })?;
        let horizontal_span = horizontal.length()?;
        Ok(Self {
            start,
            horizontal_axis,
            gravity_axis,
            horizontal_span,
            end_height,
        })
    }

    fn parameter_from_length(self, length: Real) -> Result<Real, GeometryError> {
        require_finite([length], "catenary length")?;
        let chord_length = self.horizontal_span.hypot(self.end_height);
        if length <= chord_length {
            return Err(GeometryError::InvalidCatenaryLength);
        }

        let excess_left = (length - chord_length) / self.horizontal_span;
        let excess_right = (length + chord_length) / self.horizontal_span;
        let squared_excess = excess_left * excess_right;
        let log_ratio = if squared_excess.is_finite() {
            0.5 * squared_excess.ln_1p()
        } else {
            0.5 * (excess_left.ln() + excess_right.ln())
        };
        if !(log_ratio.is_finite() && log_ratio > 0.0) {
            return Err(GeometryError::InvalidCatenaryLength);
        }

        let mut lower = 0.0;
        let mut upper = 1.0;
        while log_sinhc(upper) < log_ratio {
            upper *= 2.0;
            if !upper.is_finite() {
                return Err(GeometryError::CatenarySolveDidNotConverge);
            }
        }
        for _ in 0..ROOT_ITERATIONS {
            let middle = midpoint(lower, upper);
            if middle == lower || middle == upper {
                break;
            }
            if log_sinhc(middle) < log_ratio {
                lower = middle;
            } else {
                upper = middle;
            }
        }
        let dimensionless = midpoint(lower, upper);
        let parameter = self.horizontal_span / (2.0 * dimensionless);
        require_finite([parameter], "catenary parameter")?;
        if parameter <= 0.0 {
            return Err(GeometryError::CatenarySolveDidNotConverge);
        }
        Ok(parameter)
    }

    fn parameter_from_apex(self, picked_apex: Point3) -> Result<Real, GeometryError> {
        let desired_height = self
            .start
            .vector_to(picked_apex)?
            .dot(self.gravity_axis.as_vector())?;
        let opposite_drop = desired_height - self.end_height;
        if desired_height < 0.0
            || opposite_drop < 0.0
            || (desired_height == 0.0 && opposite_drop == 0.0)
        {
            return Err(GeometryError::InvalidCatenaryApex);
        }

        let span_for = |parameter: Real| -> Result<Real, GeometryError> {
            let span = scaled_acosh_one_plus(desired_height, parameter)
                + scaled_acosh_one_plus(opposite_drop, parameter);
            require_finite([span], "catenary apex constraint")?;
            Ok(span)
        };
        let mut lower = 0.0;
        let mut upper = self
            .horizontal_span
            .max(desired_height)
            .max(opposite_drop)
            .max(Real::MIN_POSITIVE);
        while span_for(upper)? < self.horizontal_span {
            upper *= 2.0;
            if !upper.is_finite() {
                return Err(GeometryError::CatenarySolveDidNotConverge);
            }
        }
        for _ in 0..ROOT_ITERATIONS {
            let middle = midpoint(lower, upper);
            if middle == lower || middle == upper {
                break;
            }
            if span_for(middle)? < self.horizontal_span {
                lower = middle;
            } else {
                upper = middle;
            }
        }
        let parameter = midpoint(lower, upper);
        require_finite([parameter], "catenary parameter")?;
        if parameter <= 0.0 {
            return Err(GeometryError::CatenarySolveDidNotConverge);
        }
        Ok(parameter)
    }

    fn parameter_through_point(
        self,
        through_point: Point3,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        let offset = self.start.vector_to(through_point)?;
        let horizontal = offset.dot(self.horizontal_axis.as_vector())?;
        let height = offset.dot(self.gravity_axis.as_vector())?;
        if !(0.0 < horizontal && horizontal < self.horizontal_span) {
            return Err(GeometryError::InvalidCatenaryThroughPoint);
        }
        let chord_height = self.end_height * (horizontal / self.horizontal_span);
        if height - chord_height <= tolerance.absolute() {
            return Err(GeometryError::InvalidCatenaryThroughPoint);
        }

        let residual = |parameter: Real| -> Result<Real, GeometryError> {
            let analytic = AnalyticCatenary::try_new(self, parameter)?;
            Ok(analytic.height_at_horizontal(horizontal)? - height)
        };
        let mut upper = self
            .horizontal_span
            .max(self.end_height.abs())
            .max(height.abs())
            .max(1.0);
        while residual(upper)? > 0.0 {
            upper *= 2.0;
            if !upper.is_finite() {
                return Err(GeometryError::CatenarySolveDidNotConverge);
            }
        }
        let mut lower = upper;
        loop {
            let candidate = lower * 0.5;
            if candidate == 0.0 {
                return Err(GeometryError::CatenarySolveDidNotConverge);
            }
            match residual(candidate) {
                Ok(value) if value < 0.0 => lower = candidate,
                Ok(_) | Err(GeometryError::NonFinite { .. }) => {
                    lower = candidate;
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        for _ in 0..ROOT_ITERATIONS {
            let middle = midpoint(lower, upper);
            if middle == lower || middle == upper {
                break;
            }
            match residual(middle) {
                Ok(value) if value < 0.0 => upper = middle,
                Ok(_) | Err(GeometryError::NonFinite { .. }) => lower = middle,
                Err(error) => return Err(error),
            }
        }
        let parameter = midpoint(lower, upper);
        require_finite([parameter], "catenary parameter")?;
        if parameter <= 0.0 {
            return Err(GeometryError::CatenarySolveDidNotConverge);
        }
        Ok(parameter)
    }
}

#[derive(Clone, Copy, Debug)]
struct AnalyticCatenary {
    frame: CatenaryFrame,
    parameter: Real,
    apex_horizontal: Real,
    length: Real,
}

impl AnalyticCatenary {
    fn try_new(frame: CatenaryFrame, parameter: Real) -> Result<Self, GeometryError> {
        require_finite([parameter], "catenary parameter")?;
        if parameter <= 0.0 {
            return Err(GeometryError::InvalidCatenaryParameter);
        }
        let half_argument = frame.horizontal_span / (2.0 * parameter);
        let transverse = scaled_sinhc(frame.horizontal_span, half_argument);
        require_finite([transverse], "catenary transverse length")?;
        let apex_horizontal =
            frame.horizontal_span * 0.5 + parameter * (frame.end_height / transverse).asinh();
        let length = transverse.hypot(frame.end_height);
        require_finite([apex_horizontal, length], "catenary analytic dimensions")?;
        Ok(Self {
            frame,
            parameter,
            apex_horizontal,
            length,
        })
    }

    fn uniform_points(self, count: usize) -> Result<Vec<Point3>, GeometryError> {
        debug_assert!(count >= 2);
        let mut points = Vec::with_capacity(count);
        for index in 0..count {
            if index == 0 {
                points.push(self.frame.start);
            } else if index + 1 == count {
                points.push(self.end_point()?);
            } else {
                let horizontal = self.frame.horizontal_span * index as Real / (count - 1) as Real;
                points.push(self.point_at_horizontal(horizontal)?);
            }
        }
        Ok(points)
    }

    fn end_point(self) -> Result<Point3, GeometryError> {
        self.frame
            .start
            .translated(
                self.frame
                    .horizontal_axis
                    .as_vector()
                    .scaled(self.frame.horizontal_span)?,
            )?
            .translated(
                self.frame
                    .gravity_axis
                    .as_vector()
                    .scaled(self.frame.end_height)?,
            )
    }

    fn point_at_horizontal(self, horizontal: Real) -> Result<Point3, GeometryError> {
        let height = self.height_at_horizontal(horizontal)?;
        self.frame
            .start
            .translated(self.frame.horizontal_axis.as_vector().scaled(horizontal)?)?
            .translated(self.frame.gravity_axis.as_vector().scaled(height)?)
    }

    fn height_at_horizontal(self, horizontal: Real) -> Result<Real, GeometryError> {
        require_finite([horizontal], "catenary horizontal parameter")?;
        let first_argument = horizontal / (2.0 * self.parameter);
        let second_argument = (2.0 * self.apex_horizontal - horizontal) / (2.0 * self.parameter);
        let height = scaled_sinh_product(horizontal, first_argument, second_argument);
        require_finite([height], "catenary point")?;
        Ok(height)
    }

    fn tangent_at_horizontal(self, horizontal: Real) -> Result<Vector3, GeometryError> {
        let argument = (self.apex_horizontal - horizontal) / self.parameter;
        require_finite([argument], "catenary tangent")?;
        let absolute = argument.abs();
        let horizontal_scale = if absolute < 20.0 {
            1.0 / argument.cosh()
        } else {
            let exponential = (-absolute).exp();
            2.0 * exponential / (1.0 + exponential * exponential)
        };
        combine_vectors(
            self.frame.horizontal_axis.as_vector(),
            horizontal_scale,
            self.frame.gravity_axis.as_vector(),
            argument.tanh(),
        )
    }

    fn apex(self) -> Result<Point3, GeometryError> {
        let argument = self.apex_horizontal / self.parameter;
        let half_argument = argument * 0.5;
        let apex_height =
            0.5 * self.apex_horizontal * argument * sinhc(half_argument) * sinhc(half_argument);
        require_finite([apex_height], "catenary apex")?;
        self.frame
            .start
            .translated(
                self.frame
                    .horizontal_axis
                    .as_vector()
                    .scaled(self.apex_horizontal)?,
            )?
            .translated(self.frame.gravity_axis.as_vector().scaled(apex_height)?)
    }
}

fn midpoint(lower: Real, upper: Real) -> Real {
    lower + (upper - lower) * 0.5
}

fn sinhc(value: Real) -> Real {
    let square = value * value;
    if value.abs() < 1.0e-4 {
        1.0 + square / 6.0 + square * square / 120.0
    } else {
        value.sinh() / value
    }
}

fn log_sinhc(value: Real) -> Real {
    if value < 1.0e-4 {
        let square = value * value;
        (square / 6.0 + square * square / 120.0).ln_1p()
    } else if value < 20.0 {
        (value.sinh() / value).ln()
    } else {
        value - std::f64::consts::LN_2 - value.ln() + (-(-2.0 * value).exp()).ln_1p()
    }
}

fn acosh_one_plus(value: Real) -> Real {
    if value > 1.0e150 {
        std::f64::consts::LN_2 + value.ln()
    } else {
        (value + value.sqrt() * (value + 2.0).sqrt()).ln_1p()
    }
}

fn scaled_acosh_one_plus(height: Real, parameter: Real) -> Real {
    if height == 0.0 {
        return 0.0;
    }
    let ratio = height / parameter;
    if ratio.is_finite() {
        parameter * acosh_one_plus(ratio)
    } else {
        parameter * (std::f64::consts::LN_2 + height.ln() - parameter.ln())
    }
}

fn scaled_sinhc(scale: Real, value: Real) -> Real {
    if value.abs() < 20.0 {
        scale * sinhc(value)
    } else {
        (scale.ln() + log_sinhc(value.abs())).exp()
    }
}

fn scaled_sinh_product(scale: Real, first: Real, second: Real) -> Real {
    if scale == 0.0 || second == 0.0 {
        return 0.0;
    }
    let direct = scale * sinhc(first) * second.sinh();
    if direct.is_finite() {
        return direct;
    }
    let log_second = if second.abs() < 20.0 {
        second.abs().sinh().ln()
    } else {
        second.abs() - std::f64::consts::LN_2 + (-(-2.0 * second.abs()).exp()).ln_1p()
    };
    second.signum() * (scale.ln() + log_sinhc(first.abs()) + log_second).exp()
}

fn subtract_vectors(left: Vector3, right: Vector3) -> Result<Vector3, GeometryError> {
    Vector3::try_new(
        left.x() - right.x(),
        left.y() - right.y(),
        left.z() - right.z(),
    )
}

fn combine_vectors(
    first: Vector3,
    first_scale: Real,
    second: Vector3,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn axis() -> Vector3 {
        Vector3::try_new(0.0, 0.0, -1.0).unwrap()
    }

    fn assert_near(actual: Real, expected: Real, epsilon: Real) {
        assert!(
            (actual - expected).abs() <= epsilon,
            "expected {expected:?}, got {actual:?}"
        );
    }

    fn assert_point_near(actual: Point3, expected: [Real; 3], epsilon: Real) {
        for (actual, expected) in actual.to_array().into_iter().zip(expected) {
            assert_near(actual, expected, epsilon);
        }
    }

    #[test]
    fn parameter_smooth_curve_matches_rhino_control_layout() {
        let result = try_catenary(
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            axis(),
            CatenaryConstruction::Parameter(4.0),
            CatenaryOutput::Smooth,
            8,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let CatenaryCurve::Smooth(curve) = result.curve() else {
            panic!("expected smooth output")
        };
        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.control_points().len(), 8);
        let expected = [
            [0.0, 0.0, 0.0],
            [0.548_059_652_828_782_9, 0.0, -0.877_947_215_009_473_5],
            [1.660_160_640_853_994_6, 0.0, -2.322_002_996_629_364],
            [3.862_210_616_835_685, 0.0, -3.589_638_274_557_084_5],
            [6.137_789_383_164_314, 0.0, -3.589_638_274_557_083_6],
            [8.339_839_359_146_007, 0.0, -2.322_002_996_629_364_3],
            [9.451_940_347_171_217, 0.0, -0.877_947_215_009_473_5],
            [10.0, 0.0, 0.0],
        ];
        for (control, expected) in curve.control_points().iter().zip(expected) {
            assert_point_near(control.point(), expected, 2.0e-14);
            assert_eq!(control.weight(), 1.0);
        }
        assert_near(*curve.domain().end(), 12.730_423_762_009_057, 2.0e-14);
        assert_near(result.parameter(), 4.0, 0.0);
        assert_near(result.length(), 12.815_352_642_406_605, 2.0e-14);
        assert_point_near(result.apex(), [5.0, 0.0, -3.553_695_508_644_063], 2.0e-14);
    }

    #[test]
    fn parameter_polyline_uses_uniform_horizontal_stations() {
        let result = try_catenary(
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            axis(),
            CatenaryConstruction::Parameter(4.0),
            CatenaryOutput::Polyline,
            8,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let CatenaryCurve::Polyline(polyline) = result.curve() else {
            panic!("expected polyline output")
        };
        assert_eq!(polyline.vertices().len(), 8);
        assert_point_near(
            polyline.vertices()[3],
            [4.285_714_285_714_286, 0.0, -3.489_750_346_714_287_4],
            2.0e-14,
        );
        assert_point_near(polyline.vertices()[7], [10.0, 0.0, 0.0], 0.0);
    }

    #[test]
    fn length_and_apex_modes_match_asymmetric_rhino_samples() {
        let length = try_catenary(
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, -2.0),
            axis(),
            CatenaryConstruction::Length(13.0),
            CatenaryOutput::Polyline,
            7,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_near(length.parameter(), 3.980_464_697_626_825, 2.0e-14);
        assert_near(length.length(), 13.0, 2.0e-14);
        let CatenaryCurve::Polyline(polyline) = length.curve() else {
            panic!("expected polyline output")
        };
        assert_point_near(
            polyline.vertices()[3],
            [5.0, 0.0, -4.618_680_100_732_663],
            2.0e-13,
        );

        let apex = try_catenary(
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, -2.0),
            axis(),
            CatenaryConstruction::Apex(point(7.0, 4.0, -4.0)),
            CatenaryOutput::Polyline,
            7,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_near(apex.parameter(), 4.735_537_684_366_553_5, 2.0e-10);
        assert_near(apex.apex().z(), -4.0, 2.0e-10);
        let CatenaryCurve::Polyline(polyline) = apex.curve() else {
            panic!("expected polyline output")
        };
        assert_point_near(
            polyline.vertices()[3],
            [5.0, 0.0, -3.934_292_276_445_540_4],
            2.0e-9,
        );
    }

    #[test]
    fn through_point_mode_honors_projected_constraint_and_oblique_frame() {
        let start = point(1.0, 2.0, 3.0);
        let end = point(8.0, 4.0, -1.0);
        let through = point(4.0, 2.857_142_857_142_857, -0.5);
        let result = try_catenary(
            start,
            end,
            axis(),
            CatenaryConstruction::ThroughPoint(through),
            CatenaryOutput::Polyline,
            9,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let frame = CatenaryFrame::try_new(start, end, axis(), Tolerance::DEFAULT).unwrap();
        let analytic = AnalyticCatenary::try_new(frame, result.parameter()).unwrap();
        let horizontal = start
            .vector_to(through)
            .unwrap()
            .dot(frame.horizontal_axis.as_vector())
            .unwrap();
        assert_point_near(
            analytic.point_at_horizontal(horizontal).unwrap(),
            through.to_array(),
            2.0e-12,
        );
        let CatenaryCurve::Polyline(polyline) = result.curve() else {
            panic!("expected polyline output")
        };
        assert_point_near(
            polyline.vertices()[4],
            [4.5, 3.0, -0.790_254_160_893_413_3],
            2.0e-9,
        );
    }

    #[test]
    fn apex_may_coincide_with_an_endpoint_at_minimum_point_counts() {
        let start = point(0.0, 0.0, 0.0);
        let end = point(10.0, 0.0, -2.0);
        let smooth = try_catenary(
            start,
            end,
            axis(),
            CatenaryConstruction::Apex(end),
            CatenaryOutput::Smooth,
            MIN_SMOOTH_CATENARY_POINT_COUNT,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_point_near(smooth.apex(), end.to_array(), 2.0e-12);
        let CatenaryCurve::Smooth(curve) = smooth.curve() else {
            panic!("expected smooth output")
        };
        assert_eq!(
            curve.control_points().len(),
            MIN_SMOOTH_CATENARY_POINT_COUNT
        );

        let polyline = try_catenary(
            start,
            end,
            axis(),
            CatenaryConstruction::Apex(end),
            CatenaryOutput::Polyline,
            MIN_POLYLINE_CATENARY_POINT_COUNT,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let CatenaryCurve::Polyline(polyline) = polyline.curve() else {
            panic!("expected polyline output")
        };
        assert_eq!(polyline.vertices(), &[start, end]);
    }

    #[test]
    fn rejects_degenerate_constraints_and_unsafe_point_counts() {
        let start = point(0.0, 0.0, 0.0);
        let end = point(10.0, 0.0, 0.0);
        let cases = [
            try_catenary(
                start,
                end,
                Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
                CatenaryConstruction::Parameter(4.0),
                CatenaryOutput::Smooth,
                8,
                Tolerance::DEFAULT,
            ),
            try_catenary(
                start,
                end,
                axis(),
                CatenaryConstruction::Parameter(0.0),
                CatenaryOutput::Smooth,
                8,
                Tolerance::DEFAULT,
            ),
            try_catenary(
                start,
                end,
                axis(),
                CatenaryConstruction::Length(10.0),
                CatenaryOutput::Smooth,
                8,
                Tolerance::DEFAULT,
            ),
            try_catenary(
                start,
                end,
                axis(),
                CatenaryConstruction::Apex(point(5.0, 0.0, 1.0)),
                CatenaryOutput::Smooth,
                8,
                Tolerance::DEFAULT,
            ),
            try_catenary(
                start,
                end,
                axis(),
                CatenaryConstruction::ThroughPoint(point(3.0, 0.0, 1.0)),
                CatenaryOutput::Smooth,
                8,
                Tolerance::DEFAULT,
            ),
            try_catenary(
                start,
                end,
                axis(),
                CatenaryConstruction::Parameter(4.0),
                CatenaryOutput::Smooth,
                3,
                Tolerance::DEFAULT,
            ),
            try_catenary(
                start,
                end,
                axis(),
                CatenaryConstruction::Parameter(4.0),
                CatenaryOutput::Polyline,
                MAX_CATENARY_POINT_COUNT + 1,
                Tolerance::DEFAULT,
            ),
        ];
        assert!(cases.into_iter().all(|result| result.is_err()));

        assert!(
            try_catenary(
                start,
                point(0.0, 0.0, -5.0),
                axis(),
                CatenaryConstruction::Parameter(4.0),
                CatenaryOutput::Smooth,
                8,
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }
}
