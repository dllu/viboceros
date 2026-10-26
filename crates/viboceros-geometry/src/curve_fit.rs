use crate::{
    CurveRef, GeometryError, NurbsCurve, Point3, Real, Tolerance,
    curve::{ArcLengthKink, ArcLengthSampler},
};

mod adaptive;
use adaptive::try_fit_curve_impl;
pub(crate) mod g1;
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

fn stable_midpoint(start: Real, end: Real) -> Real {
    0.5 * start + 0.5 * end
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod constraints_tests;

#[cfg(test)]
mod station_tests;
