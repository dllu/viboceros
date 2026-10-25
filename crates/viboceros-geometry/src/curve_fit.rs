use std::collections::HashMap;

use crate::{
    CurveRef, GeometryError, NurbsCurve, Point3, Real, Tolerance,
    curve::{ArcLengthKink, ArcLengthSampler},
};

mod interpolate;
use interpolate::interpolate_fit;
mod sampler;
use sampler::{FitKink, FitSampler};

/// Largest degree accepted by Rhino's `FitCrv` command.
pub const MAX_CURVE_FIT_DEGREE: usize = 11;

/// Resource ceiling for adaptive cubic curve fitting with banded solves.
/// A full radius-three circle at the default Sweep1 fit tolerance needs 515
/// controls; larger circles need more at the same absolute tolerance. Other
/// degrees keep the smaller dense-solve ceiling below.
pub const MAX_CURVE_FIT_CONTROL_POINTS: usize = 4096;
const MAX_DENSE_CURVE_FIT_CONTROL_POINTS: usize = 512;

const CURVE_FIT_ERROR_SAMPLES_PER_SPAN: usize = 16;
const MAX_CACHED_FIT_POINTS: usize = 32_768;

/// Shared fixed-row collocation. A nonzero mask row fixes that control directly;
/// remaining rows interpolate at the supplied stations. Targets use local
/// homogeneous channels; polynomial curve fits supply weights equal to one.
pub struct FixedInterpolationSystem<'a> {
    pub system: crate::NurbsInterpolationSystem<'a>,
    pub fixed_rows: &'a [u8],
}
pub type CurveFitInterpolation<'a> = FixedInterpolationSystem<'a>;
type FitSolver<'a, E> = dyn FnMut(&CurveFitInterpolation<'_>) -> Result<Vec<Real>, E> + 'a;

/// A position constraint at an original source parameter. Endpoints and kinks
/// retain their natural positions; other targets may move by the fit tolerance.
#[derive(Clone, Copy, Debug)]
pub struct CurveFitConstraint {
    pub source_parameter: Real,
    pub point: Point3,
}

/// Polynomial curve and its parameters for the requested constraints, in the
/// caller's original order. Arc-length fits return source arc-length stations.
#[derive(Clone, Debug)]
pub struct ConstrainedCurveFit {
    pub curve: NurbsCurve,
    pub constraint_parameters: Vec<Real>,
}

/// Position constraint with a requested unit output parameter. A common map
/// requires both original endpoints at 0 and 1 and strictly increasing stations.
#[derive(Clone, Copy, Debug)]
pub struct CurveFitStation {
    pub source_parameter: Real,
    pub fit_parameter: Real,
    pub point: Point3,
}

struct FitConstraints<'a> {
    points: &'a [CurveFitConstraint],
    parameters: Option<&'a [Real]>,
}
impl<'a> FitConstraints<'a> {
    fn arc_length(points: &'a [CurveFitConstraint]) -> Self {
        Self {
            points,
            parameters: None,
        }
    }
}

#[derive(Clone, Copy)]
struct Pin {
    distance: Real,
    point: Point3,
}
const MAX_FIT_CONSTRAINTS: usize = 64;

/// Cubic arc-length fitting with interpolated positions. Simple constraint
/// knots retain C2 continuity away from retained source kinks. Fit error uses
/// the same adaptive sampled policy as `try_fit_curve`; constraint positions
/// additionally require an exact stored-curve bound at numerical tolerance.
pub fn try_fit_cubic_curve_with_constraints(
    source: CurveRef<'_>,
    constraints: &[CurveFitConstraint],
    fit_tolerance: Real,
    angle_tolerance_radians: Real,
    numerical_tolerance: Tolerance,
) -> Result<ConstrainedCurveFit, GeometryError> {
    try_fit_curve_impl(
        source,
        3,
        fit_tolerance,
        angle_tolerance_radians,
        numerical_tolerance,
        FitConstraints::arc_length(constraints),
        None,
    )
}

/// Use a caller's fixed-row interpolation backend for constrained cubic fits.
pub fn try_fit_cubic_curve_with_constraints_with_solver<E: From<GeometryError>>(
    source: CurveRef<'_>,
    constraints: &[CurveFitConstraint],
    fit_tolerance: Real,
    angle_tolerance_radians: Real,
    numerical_tolerance: Tolerance,
    solve: &mut impl FnMut(&CurveFitInterpolation<'_>) -> Result<Vec<Real>, E>,
) -> Result<ConstrainedCurveFit, E> {
    try_fit_curve_impl(
        source,
        3,
        fit_tolerance,
        angle_tolerance_radians,
        numerical_tolerance,
        FitConstraints::arc_length(constraints),
        Some(solve),
    )
}

/// Fit a cubic in a common unit parameter chart. An interpolating scalar map
/// from the requested stations to normalized source arc length uses degree
/// min(3, station_count-1). Positive derivative controls must prove that it is
/// strictly increasing. The adaptive fit, fixed handles and exact position
/// checks are shared with the arc-length fitter.
pub fn try_fit_cubic_curve_at_stations(
    source: CurveRef<'_>,
    stations: &[CurveFitStation],
    fit_tolerance: Real,
    angle_tolerance_radians: Real,
    numerical_tolerance: Tolerance,
) -> Result<ConstrainedCurveFit, GeometryError> {
    fit_at_stations(
        source,
        stations,
        fit_tolerance,
        angle_tolerance_radians,
        numerical_tolerance,
        None,
    )
}

/// Common-parameter fitting with the caller's fixed-row interpolation backend.
pub fn try_fit_cubic_curve_at_stations_with_solver<E: From<GeometryError>>(
    source: CurveRef<'_>,
    stations: &[CurveFitStation],
    fit_tolerance: Real,
    angle_tolerance_radians: Real,
    numerical_tolerance: Tolerance,
    solve: &mut impl FnMut(&CurveFitInterpolation<'_>) -> Result<Vec<Real>, E>,
) -> Result<ConstrainedCurveFit, E> {
    fit_at_stations(
        source,
        stations,
        fit_tolerance,
        angle_tolerance_radians,
        numerical_tolerance,
        Some(solve),
    )
}

fn fit_at_stations<E: From<GeometryError>>(
    source: CurveRef<'_>,
    stations: &[CurveFitStation],
    fit_tolerance: Real,
    angle_tolerance_radians: Real,
    numerical_tolerance: Tolerance,
    solve: Option<&mut FitSolver<'_, E>>,
) -> Result<ConstrainedCurveFit, E> {
    if !(2..=MAX_FIT_CONSTRAINTS).contains(&stations.len()) {
        return Err(GeometryError::InvalidCurveFitConstraints {
            context: "2 to 64 stations including both endpoints are required",
        }
        .into());
    }
    let constraints = stations
        .iter()
        .map(|s| CurveFitConstraint {
            source_parameter: s.source_parameter,
            point: s.point,
        })
        .collect::<Vec<_>>();
    let parameters = stations.iter().map(|s| s.fit_parameter).collect::<Vec<_>>();
    try_fit_curve_impl(
        source,
        3,
        fit_tolerance,
        angle_tolerance_radians,
        numerical_tolerance,
        FitConstraints {
            points: &constraints,
            parameters: Some(&parameters),
        },
        solve,
    )
}

#[derive(Clone, Copy, Debug)]
struct FitBreak {
    distance: Real,
    multiplicity: usize,
}

/// Approximates a curve with a non-rational NURBS curve of the requested
/// degree, using arc length as the output parameter.
///
/// Natural source-span boundaries seed the adaptive approximation. Boundaries
/// whose one-sided tangents differ by more than `angle_tolerance_radians` are
/// retained as C0 knots; the output also preserves one-sided tangents there and
/// at the natural endpoints. Failing spans are bisected, prioritized by their
/// sampled errors, until the tolerance or resource ceiling is reached.
pub fn try_fit_curve(
    source: CurveRef<'_>,
    degree: usize,
    fit_tolerance: Real,
    angle_tolerance_radians: Real,
    numerical_tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    try_fit_curve_impl(
        source,
        degree,
        fit_tolerance,
        angle_tolerance_radians,
        numerical_tolerance,
        FitConstraints::arc_length(&[]),
        None,
    )
    .map(|result| result.curve)
}

/// Retain adaptive fitting and handle policy while delegating control solving
/// to the caller's basis/interpolation backend. Degree-one paths stay direct.
pub fn try_fit_curve_with_solver<E: From<GeometryError>>(
    source: CurveRef<'_>,
    degree: usize,
    fit_tolerance: Real,
    angle_tolerance_radians: Real,
    numerical_tolerance: Tolerance,
    solve: &mut impl FnMut(&CurveFitInterpolation<'_>) -> Result<Vec<Real>, E>,
) -> Result<NurbsCurve, E> {
    try_fit_curve_impl(
        source,
        degree,
        fit_tolerance,
        angle_tolerance_radians,
        numerical_tolerance,
        FitConstraints::arc_length(&[]),
        Some(solve),
    )
    .map(|result| result.curve)
}

fn try_fit_curve_impl<E: From<GeometryError>>(
    source: CurveRef<'_>,
    degree: usize,
    fit_tolerance: Real,
    angle_tolerance_radians: Real,
    numerical_tolerance: Tolerance,
    constraints: FitConstraints<'_>,
    mut solve: Option<&mut FitSolver<'_, E>>,
) -> Result<ConstrainedCurveFit, E> {
    if degree == 0 || degree > MAX_CURVE_FIT_DEGREE {
        return Err(GeometryError::InvalidCurveFitDegree {
            actual: degree,
            maximum: MAX_CURVE_FIT_DEGREE,
        }
        .into());
    }
    if !fit_tolerance.is_finite() || fit_tolerance <= 0.0 {
        return Err(GeometryError::InvalidCurveFitTolerance.into());
    }
    if !angle_tolerance_radians.is_finite()
        || !(0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians)
    {
        return Err(GeometryError::InvalidCurveFitAngleTolerance.into());
    }
    let maximum_controls = if degree == 3 {
        MAX_CURVE_FIT_CONTROL_POINTS
    } else {
        MAX_DENSE_CURVE_FIT_CONTROL_POINTS
    };

    if constraints.points.len() > MAX_FIT_CONSTRAINTS {
        return Err(GeometryError::InvalidCurveFitConstraints {
            context: "at most 64 constraints are supported",
        }
        .into());
    }
    let mut sampler = ArcLengthSampler::try_new(source, numerical_tolerance)?;
    sampler.prepare_budgeted_repeated_sampling(32)?;
    let total_length = sampler.total_length();
    let kinks = sampler.kinks(angle_tolerance_radians)?;
    let mut pins = Vec::with_capacity(constraints.points.len());
    let mut constraint_parameters = Vec::with_capacity(constraints.points.len());
    for pin in constraints.points {
        let distance = sampler.distance_at_parameter(pin.source_parameter)?;
        let point = source.evaluate(pin.source_parameter)?;
        if point.distance_to(pin.point)? > fit_tolerance {
            return Err(GeometryError::InvalidCurveFitConstraints {
                context: "target exceeds the fit tolerance from its source point",
            }
            .into());
        }
        let fixed = distance == 0.
            || distance == total_length
            || kinks.iter().any(|k| k.distance == distance);
        if fixed && point.distance_to(pin.point)? > numerical_tolerance.absolute() {
            return Err(GeometryError::InvalidCurveFitConstraints {
                context: "endpoints and retained kinks keep their source positions",
            }
            .into());
        }
        pins.push(Pin {
            distance,
            point: pin.point,
        });
        constraint_parameters.push(distance);
    }
    pins.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    if pins.windows(2).any(|p| p[0].distance == p[1].distance) {
        return Err(GeometryError::InvalidCurveFitConstraints {
            context: "constraints must have distinct arc-length stations",
        }
        .into());
    }

    let natural_breaks = sampler.natural_break_distances().collect::<Vec<_>>();
    if natural_breaks.len() > maximum_controls.saturating_sub(degree + 1) {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: maximum_controls,
        }
        .into());
    }
    let sampler = FitSampler::try_new(sampler, &constraint_parameters, constraints.parameters)?;
    for pin in &mut pins {
        pin.distance = sampler.parameter_at_distance(pin.distance)?;
    }
    for parameter in &mut constraint_parameters {
        *parameter = sampler.parameter_at_distance(*parameter)?;
    }
    let kinks = sampler.map_kinks(&kinks)?;
    let total_length = sampler.end_parameter();
    let mut breaks = natural_breaks
        .into_iter()
        .map(|distance| {
            let parameter = sampler.parameter_at_distance(distance)?;
            Ok(FitBreak {
                distance: parameter,
                multiplicity: if kinks.iter().any(|k| k.parameter == parameter) {
                    degree
                } else {
                    1
                },
            })
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;

    if constraints.parameters.is_some()
        && (breaks
            .iter()
            .any(|b| b.distance <= 0. || b.distance >= total_length)
            || breaks.windows(2).any(|p| p[0].distance >= p[1].distance))
    {
        return Err(GeometryError::InvalidCurveFitConstraints {
            context: "common parameter map collapsed a natural source span",
        }
        .into());
    }
    for pin in &pins {
        if pin.distance > 0. && pin.distance < total_length {
            insert_smooth_break(&mut breaks, pin.distance);
        }
    }

    // A quadratic needs two knot spans between consecutive tangent-constrained
    // boundaries so the incoming and outgoing handle controls do not overlap.
    if degree == 2 {
        let mut hard_boundaries = Vec::with_capacity(kinks.len() + 2);
        hard_boundaries.push(0.0);
        hard_boundaries.extend(kinks.iter().map(|kink| kink.parameter));
        hard_boundaries.push(total_length);
        for interval in hard_boundaries.windows(2) {
            if !breaks
                .iter()
                .any(|item| item.distance > interval[0] && item.distance < interval[1])
            {
                let midpoint = stable_midpoint(interval[0], interval[1]);
                if midpoint <= interval[0] || midpoint >= interval[1] {
                    return Err(GeometryError::Degenerate {
                        context: "quadratic curve fit span",
                    }
                    .into());
                }
                breaks.push(FitBreak {
                    distance: midpoint,
                    multiplicity: 1,
                });
            }
        }
        breaks.sort_by(|left, right| left.distance.total_cmp(&right.distance));
    }

    // Balanced refinement revisits many identical source distances. Cache
    // exact floating-point keys, without quantizing stations or changing the
    // integration tolerance. The cache is local to this fit and bounded.
    let mut points = HashMap::new();
    loop {
        let control_count = fit_control_count(degree, &breaks, maximum_controls)?;
        if control_count > maximum_controls {
            return Err(GeometryError::TooManyCurveFitControlPoints {
                maximum: maximum_controls,
            }
            .into());
        }
        let knots = fit_knots(degree, total_length, &breaks, control_count);
        let approximation = interpolate_fit(&sampler, degree, &knots, &kinks, &pins, &mut solve)?;
        let errors =
            sampled_span_errors(&sampler, &approximation, total_length, &breaks, &mut points)?;
        let maximum_deviation = errors
            .iter()
            .map(|error| error.deviation)
            .fold(0.0_f64, Real::max);
        if maximum_deviation <= fit_tolerance {
            let stations = pins
                .iter()
                .map(|p| (p.distance, crate::ParameterSide::Right, p.point))
                .collect::<Vec<_>>();
            if !stations.is_empty()
                && !crate::surface_pullback::edit_point_bounds(
                    &approximation,
                    &stations,
                    numerical_tolerance.absolute(),
                )?
            {
                return Err(GeometryError::CurveFitConstraintDidNotConverge {
                    tolerance: numerical_tolerance.absolute(),
                }
                .into());
            }
            return Ok(ConstrainedCurveFit {
                curve: approximation,
                constraint_parameters,
            });
        }

        let available = maximum_controls - control_count;
        if available == 0 {
            return Err(GeometryError::CurveFitDidNotConverge {
                tolerance: fit_tolerance,
                deviation: maximum_deviation,
                maximum: maximum_controls,
            }
            .into());
        }
        let mut refinements = errors
            .into_iter()
            .filter(|error| error.deviation > fit_tolerance)
            .collect::<Vec<_>>();
        refinements.sort_by(|left, right| right.deviation.total_cmp(&left.deviation));
        refinements.truncate(available);
        refinements.sort_by(|left, right| left.refinement.total_cmp(&right.refinement));

        let previous_len = breaks.len();
        for refinement in refinements {
            insert_smooth_break(&mut breaks, refinement.refinement);
        }
        if breaks.len() == previous_len {
            return Err(GeometryError::CurveFitDidNotConverge {
                tolerance: fit_tolerance,
                deviation: maximum_deviation,
                maximum: maximum_controls,
            }
            .into());
        }
    }
}

fn fit_control_count(
    degree: usize,
    breaks: &[FitBreak],
    maximum_controls: usize,
) -> Result<usize, GeometryError> {
    breaks
        .iter()
        .try_fold(degree + 1, |count, item| {
            count.checked_add(item.multiplicity)
        })
        .ok_or(GeometryError::TooManyCurveFitControlPoints {
            maximum: maximum_controls,
        })
}

fn fit_knots(
    degree: usize,
    total_length: Real,
    breaks: &[FitBreak],
    control_count: usize,
) -> Vec<Real> {
    let mut knots = Vec::with_capacity(control_count + degree + 1);
    knots.extend(std::iter::repeat_n(0.0, degree + 1));
    for item in breaks {
        knots.extend(std::iter::repeat_n(item.distance, item.multiplicity));
    }
    knots.extend(std::iter::repeat_n(total_length, degree + 1));
    knots
}

#[derive(Clone, Copy, Debug)]
struct SpanError {
    deviation: Real,
    refinement: Real,
}

fn sampled_span_errors(
    sampler: &FitSampler<'_>,
    approximation: &NurbsCurve,
    total_length: Real,
    breaks: &[FitBreak],
    points: &mut HashMap<u64, Point3>,
) -> Result<Vec<SpanError>, GeometryError> {
    let mut boundaries = Vec::with_capacity(breaks.len() + 2);
    boundaries.push(0.0);
    boundaries.extend(breaks.iter().map(|item| item.distance));
    boundaries.push(total_length);

    boundaries
        .windows(2)
        .map(|interval| {
            let mut largest = SpanError {
                deviation: 0.0,
                refinement: stable_midpoint(interval[0], interval[1]),
            };
            for sample_index in 1..CURVE_FIT_ERROR_SAMPLES_PER_SPAN {
                let fraction = sample_index as Real / CURVE_FIT_ERROR_SAMPLES_PER_SPAN as Real;
                let parameter = interval[0].mul_add(1.0 - fraction, interval[1] * fraction);
                let exact = if let Some(&point) = points.get(&parameter.to_bits()) {
                    point
                } else {
                    let point = sampler.point_at_parameter(parameter)?;
                    if points.len() < MAX_CACHED_FIT_POINTS {
                        points.insert(parameter.to_bits(), point);
                    }
                    point
                };
                let fitted = approximation.evaluate(parameter)?;
                let deviation = exact.distance_to(fitted)?;
                if deviation > largest.deviation {
                    largest.deviation = deviation;
                }
            }
            Ok(largest)
        })
        .collect()
}

fn insert_smooth_break(breaks: &mut Vec<FitBreak>, parameter: Real) {
    match breaks.binary_search_by(|item| item.distance.total_cmp(&parameter)) {
        Ok(_) => {}
        Err(index) => breaks.insert(
            index,
            FitBreak {
                distance: parameter,
                multiplicity: 1,
            },
        ),
    }
}

fn stable_midpoint(start: Real, end: Real) -> Real {
    0.5 * start + 0.5 * end
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod constraints_tests;

#[cfg(test)]
mod station_tests;
