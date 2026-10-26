//! Native collocation producers, fitting and point-grid interpolation.
use super::*;
mod point_grid;
#[cfg(test)]
use point_grid::solve_point_grid;
pub use point_grid::{interpolate_open_point_grid, interpolate_point_grid, point_grid_brep};

pub(super) fn solve_cubic_system(
    input: &viboceros_geometry::CubicInterpolationSystem<'_>,
) -> Result<Vec<f64>, Error> {
    let view = InterpolationSystem {
        knots: input.knots.as_ptr(),
        parameters: input.parameters.as_ptr(),
        targets: input.targets.as_ptr(),
        knot_count: input.knots.len(),
        parameter_count: input.parameters.len(),
        columns: input.columns,
        target_count: input.targets.len(),
        degree: 3,
        closed: c_int::from(input.closed),
        handles: c_int::from(!input.closed),
        uniform_periodic: 0,
        fixed_rows: std::ptr::null(),
        fixed_count: 0,
        point_grid: 0,
        amplification: std::ptr::null_mut(),
        amplification_count: 0,
        nonuniform_periodic: 0,
        periodic_control_shift: 0,
        derivative_orders: std::ptr::null(),
        derivative_count: 0,
    };
    let mut output = vec![0.; input.targets.len()];
    call(|error, capacity| unsafe {
        vb_interpolation_solve(&view, output.as_mut_ptr(), output.len(), error, capacity)
    })?;
    Ok(output)
}

/// Uniform open/periodic Rebuild with shared arc-length sampling and tangent
/// projection, using the native banded basis and matrix routines.
pub fn rebuild_curve(
    source: viboceros_geometry::CurveRef<'_>,
    point_count: usize,
    degree: usize,
    preserve_end_tangents: bool,
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    viboceros_geometry::try_rebuild_curve_with_interpolation(
        source,
        point_count,
        degree,
        preserve_end_tangents,
        tolerance,
        &mut solve_rebuild_interpolation,
    )
}

pub(super) fn solve_rebuild_interpolation(
    input: &viboceros_geometry::CurveRebuildInterpolation<'_>,
) -> Result<Vec<[f64; 3]>, Error> {
    let targets = input
        .targets
        .iter()
        .flat_map(|p| [p[0], p[1], p[2], 1.])
        .collect::<Vec<_>>();
    let output = solve_uniform_system(&viboceros_geometry::UniformInterpolationSystem {
        knots: input.knots,
        parameters: input.parameters,
        targets: &targets,
        columns: 4,
        degree: input.degree,
        periodic: input.periodic,
    })?;
    Ok(output.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect())
}

pub fn interpolate_affine_greville(
    source: &NurbsCurve,
    point_count: usize,
) -> Result<NurbsCurve, Error> {
    viboceros_geometry::try_interpolate_affine_greville_with_solver(
        source,
        point_count,
        &mut solve_rebuild_interpolation,
    )
}

/// Match structural preparation uses checked native trimming, elevation,
/// refinement and affine Greville solving around the shared continuity policy.
pub fn match_curve_end(
    source: (&viboceros_geometry::Curve3, bool),
    reference: (&viboceros_geometry::Curve3, bool),
    continuity: viboceros_geometry::CurveBlendContinuity,
    preserve: viboceros_geometry::CurveMatchPreserveEnd,
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    with_match_operations(|operations| {
        viboceros_geometry::try_match_curve_end_with_operations(
            source, reference, continuity, preserve, tolerance, operations,
        )
    })
}

pub fn average_match_curve_ends(
    first: (&viboceros_geometry::Curve3, bool),
    second: (&viboceros_geometry::Curve3, bool),
    continuity: viboceros_geometry::CurveBlendContinuity,
    preserve: viboceros_geometry::CurveMatchPreserveEnd,
    tolerance: Tolerance,
) -> Result<(NurbsCurve, NurbsCurve), Error> {
    with_match_operations(|operations| {
        viboceros_geometry::try_average_match_curve_ends_with_operations(
            first, second, continuity, preserve, tolerance, operations,
        )
    })
}

fn with_match_operations<T>(
    run: impl FnOnce(&mut viboceros_geometry::CurveMatchOperations<'_, Error>) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut trim = trim_curve;
    let mut elevate = |curve: &NurbsCurve, degree| change_curve_degree(curve, degree, false);
    let mut refine = insert_curve_knot;
    let mut affine = interpolate_affine_greville;
    run(&mut viboceros_geometry::CurveMatchOperations {
        trim: &mut trim,
        elevate: &mut elevate,
        refine: &mut refine,
        affine_greville: &mut affine,
    })
}

/// Rebuild a uniform tensor through native station/pole isocurves and batched
/// native banded solves, retaining shared sampling and topology policy.
pub fn rebuild_surface(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
) -> Result<NurbsSurface, Error> {
    viboceros_geometry::try_rebuild_nurbs_surface_with_operations(
        source,
        count,
        degree,
        &mut surface_isocurve,
        &mut solve_uniform_system,
    )
}

fn solve_uniform_system(
    input: &viboceros_geometry::UniformInterpolationSystem<'_>,
) -> Result<Vec<f64>, Error> {
    solve_with_fixed_rows(input, None)
}

/// Native fixed-row interpolation for adaptive morphs and display cages.
pub fn morph_interpolation(
    input: &viboceros_geometry::FixedInterpolationSystem<'_>,
) -> Result<Vec<f64>, GeometryError> {
    solve_with_fixed_rows(&input.system, Some(input.fixed_rows)).map_err(|error| match error {
        Error::Geometry(error) => error,
        error => GeometryError::InterpolationBackendFailed {
            message: error.to_string(),
        },
    })
}

/// Original Greville edit-point systems reuse native fixed-row or nonuniform
/// periodic collocation, retaining shared reconstruction and exact admission.
pub fn curve_edit_point_interpolation(
    input: &viboceros_geometry::CurveEditPointInterpolation<'_>,
) -> Result<Vec<f64>, GeometryError> {
    solve_interpolation(
        &input.system.system,
        if input.periodic_shift.is_some() {
            None
        } else {
            Some(input.system.fixed_rows)
        },
        input.periodic_shift,
    )
    .map_err(|error| match error {
        Error::Geometry(e) => e,
        error => GeometryError::InterpolationBackendFailed {
            message: error.to_string(),
        },
    })
}

pub(super) fn solve_with_fixed_rows(
    input: &viboceros_geometry::NurbsInterpolationSystem<'_>,
    fixed: Option<&[u8]>,
) -> Result<Vec<f64>, Error> {
    solve_interpolation(input, fixed, None)
}

pub(super) fn solve_interpolation(
    input: &viboceros_geometry::NurbsInterpolationSystem<'_>,
    fixed: Option<&[u8]>,
    nonuniform_periodic: Option<usize>,
) -> Result<Vec<f64>, Error> {
    let view = InterpolationSystem {
        knots: input.knots.as_ptr(),
        parameters: input.parameters.as_ptr(),
        targets: input.targets.as_ptr(),
        knot_count: input.knots.len(),
        parameter_count: input.parameters.len(),
        columns: input.columns,
        target_count: input.targets.len(),
        degree: input.degree,
        closed: c_int::from(input.periodic),
        handles: 0,
        uniform_periodic: c_int::from(input.periodic && nonuniform_periodic.is_none()),
        fixed_rows: fixed.map_or(std::ptr::null(), |f| f.as_ptr()),
        fixed_count: fixed.map_or(0, |f| f.len()),
        point_grid: 0,
        amplification: std::ptr::null_mut(),
        amplification_count: 0,
        nonuniform_periodic: c_int::from(nonuniform_periodic.is_some()),
        periodic_control_shift: nonuniform_periodic.unwrap_or(0),
        derivative_orders: std::ptr::null(),
        derivative_count: 0,
    };
    let mut output = vec![0.; input.targets.len()];
    call(|error, capacity| unsafe {
        vb_interpolation_solve(&view, output.as_mut_ptr(), output.len(), error, capacity)
    })?;
    Ok(output)
}

/// Solve exact mixed endpoint jets for all blend columns in one native job.
pub fn interpolate_endpoints(
    input: &viboceros_geometry::EndpointInterpolation<'_>,
) -> Result<Vec<[f64; 4]>, Error> {
    input.validate()?;
    let degree = input.degree();
    let first = usize::from(input.orders[0]);
    let second = usize::from(input.orders[1]);
    let parameters = std::iter::repeat_n(0., first + 1)
        .chain(std::iter::repeat_n(1., second + 1))
        .collect::<Vec<_>>();
    let orders = (0..=first)
        .chain((0..=second).rev())
        .map(|order| order as u8)
        .collect::<Vec<_>>();
    let knots = std::iter::repeat_n(0., degree + 1)
        .chain(std::iter::repeat_n(1., degree + 1))
        .collect::<Vec<_>>();
    let targets = input.targets.iter().flat_map(|h| *h).collect::<Vec<_>>();
    let view = InterpolationSystem {
        knots: knots.as_ptr(),
        parameters: parameters.as_ptr(),
        targets: targets.as_ptr(),
        knot_count: knots.len(),
        parameter_count: parameters.len(),
        columns: input.column_count * 4,
        target_count: targets.len(),
        degree,
        closed: 0,
        handles: 0,
        uniform_periodic: 0,
        fixed_rows: std::ptr::null(),
        fixed_count: 0,
        point_grid: 0,
        amplification: std::ptr::null_mut(),
        amplification_count: 0,
        nonuniform_periodic: 0,
        periodic_control_shift: 0,
        derivative_orders: orders.as_ptr(),
        derivative_count: orders.len(),
    };
    let mut output = vec![0.; targets.len()];
    call(|error, capacity| unsafe {
        vb_interpolation_solve(&view, output.as_mut_ptr(), output.len(), error, capacity)
    })?;
    Ok(output
        .chunks_exact(4)
        .map(|h| [h[0], h[1], h[2], h[3]])
        .collect())
}

/// Exact surface boundary interpolation through the shared endpoint solver.
pub fn interpolate_surface_blend(
    input: &viboceros_geometry::SurfaceBlendInterpolation<'_>,
) -> Result<Vec<[f64; 4]>, Error> {
    interpolate_endpoints(&input.endpoint_interpolation())
}

/// Smooth a closed curve through native nonuniform periodic collocation.
/// Direct cyclic control retention for Smooth=No stays in the shared core.
/// Degrees above the native limit and systems above 1,000 unique controls use
/// the portable producer; supported native failures propagate without retry.
pub fn make_curve_periodic(source: &NurbsCurve, smooth: bool) -> Result<NurbsCurve, Error> {
    if source.degree() > 17 || source.control_points().len() - source.degree() > 1000 {
        return Ok(source.try_make_periodic(smooth)?);
    }
    source.try_make_periodic_with_solver(smooth, &mut |input, shift| {
        solve_interpolation(input, None, Some(shift))
    })
}

/// Apply the same native periodic solver to requested surface control rows.
/// A requested direction outside the native degree/count bounds uses the shared
/// portable surface producer, preserving its closure and both-axis policy.
pub fn make_surface_periodic(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    smooth: bool,
) -> Result<NurbsSurface, Error> {
    let unsupported = |degree, count| degree > 17 || count - degree > 1000;
    if (direction != SurfaceKnotDirection::V
        && unsupported(source.degree_u(), source.control_point_count_u()))
        || (direction != SurfaceKnotDirection::U
            && unsupported(source.degree_v(), source.control_point_count_v()))
    {
        return Ok(source.try_make_periodic(direction, smooth)?);
    }
    source.try_make_periodic_with_solver(direction, smooth, &mut |input, shift| {
        solve_interpolation(input, None, Some(shift))
    })
}

/// Adaptive degree-one/cubic/general polynomial fit, with shared arc-length,
/// kink, tangent and refinement policies around native fixed-row solving.
pub fn fit_curve(
    source: viboceros_geometry::CurveRef<'_>,
    degree: usize,
    fit_tolerance: f64,
    angle_tolerance: f64,
    numerical_tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    viboceros_geometry::try_fit_curve_with_solver(
        source,
        degree,
        fit_tolerance,
        angle_tolerance,
        numerical_tolerance,
        &mut |input| solve_with_fixed_rows(&input.system, Some(input.fixed_rows)),
    )
}

/// Cubic arc-length fitting that interpolates given crossing points using the
/// existing native fixed-row collocation backend and exact constraint checks.
pub fn fit_cubic_curve_with_constraints(
    source: viboceros_geometry::CurveRef<'_>,
    constraints: &[viboceros_geometry::CurveFitConstraint],
    fit_tolerance: f64,
    angle_tolerance: f64,
    numerical_tolerance: Tolerance,
) -> Result<viboceros_geometry::ConstrainedCurveFit, Error> {
    viboceros_geometry::try_fit_cubic_curve_with_constraints_with_solver(
        source,
        constraints,
        fit_tolerance,
        angle_tolerance,
        numerical_tolerance,
        &mut |input| solve_with_fixed_rows(&input.system, Some(input.fixed_rows)),
    )
}

/// Cubic fitting in a common unit parameter with shared smooth arc-length
/// station maps and native fixed-row collocation.
pub fn fit_cubic_curve_at_stations(
    source: viboceros_geometry::CurveRef<'_>,
    stations: &[viboceros_geometry::CurveFitStation],
    fit_tolerance: f64,
    angle_tolerance: f64,
    numerical_tolerance: Tolerance,
) -> Result<viboceros_geometry::ConstrainedCurveFit, Error> {
    viboceros_geometry::try_fit_cubic_curve_at_stations_with_solver(
        source,
        stations,
        fit_tolerance,
        angle_tolerance,
        numerical_tolerance,
        &mut |input| solve_with_fixed_rows(&input.system, Some(input.fixed_rows)),
    )
}

/// Interpolate an open mean-chord point grid using the existing native basis
/// and batched banded solve; degree/count and residual policies stay shared.
/// Control-net tweens, with forced-open native rebuilding repeated before
/// each output when source degrees or counts differ.
pub fn surface_tweens(
    start: &NurbsSurface,
    end: &NurbsSurface,
    number: usize,
) -> Result<Vec<NurbsSurface>, Error> {
    viboceros_geometry::try_tween_nurbs_surfaces_with_rebuild(start, end, number, &mut |s, c, d| {
        viboceros_geometry::try_rebuild_open_nurbs_surface_with_operations(
            s,
            c,
            d,
            &mut surface_isocurve,
            &mut solve_uniform_system,
        )
    })
}

/// Refit uses native clamping/elevation/refinement around the shared exact
/// knot union, rational displacement and end-source domain policy.
pub fn surface_tweens_refitted(
    start: &NurbsSurface,
    end: &NurbsSurface,
    number: usize,
) -> Result<Vec<NurbsSurface>, Error> {
    viboceros_geometry::try_tween_nurbs_surfaces_refitted_with_operations(
        start,
        end,
        number,
        &mut |s| {
            clamp_surface_direction(
                &clamp_surface_direction(s, SurfaceKnotDirection::U)?,
                SurfaceKnotDirection::V,
            )
        },
        &mut tween_elevate_surface,
        &mut insert_surface_knot,
    )
}

fn tween_elevate_surface(source: &NurbsSurface, degree: [usize; 2]) -> Result<NurbsSurface, Error> {
    change_surface_degree(source, degree[0], degree[1], false)
}

/// Sampled UV tweens use the shared correspondence/blend policy and native
/// open mean-chord grid interpolation for every output.
pub fn surface_tweens_sampled(
    start: &NurbsSurface,
    end: &NurbsSurface,
    number: usize,
    sample_number: usize,
) -> Result<Vec<NurbsSurface>, Error> {
    viboceros_geometry::try_tween_nurbs_surfaces_sampled_with_grid(
        start,
        end,
        number,
        sample_number,
        &mut interpolate_open_point_grid,
    )
}

/// Interpolate a point set with shared SDK degree/closure policy and native
/// cubic basis evaluation and solving. Direct degree-one/two-point paths remain exact.
pub fn interpolate_curve(
    points: &[Point3],
    options: viboceros_geometry::CurveInterpolationOptions,
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    NurbsCurve::try_interpolate_with_solver(points, options, tolerance, &mut solve_cubic_system)
}

/// Preserve InterpCrv's exact coincidence and two-point cubic command behavior.
pub fn interpolate_curve_for_command(
    points: &[Point3],
    options: viboceros_geometry::CurveInterpolationOptions,
) -> Result<NurbsCurve, Error> {
    NurbsCurve::try_interpolate_for_command_with_solver(points, options, &mut solve_cubic_system)
}

/// TweenCurves with the shared direct blend/refit policies and native banded
/// construction of sampled source cubics through the full 9,999-division range.
pub fn curve_tweens(
    start: &NurbsCurve,
    end: &NurbsCurve,
    number: usize,
    method: viboceros_geometry::CurveTweenMatchMethod,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, Error> {
    viboceros_geometry::try_tween_nurbs_curves_with_sampling(
        start,
        end,
        number,
        method,
        tolerance,
        &mut |points, options, maximum| {
            NurbsCurve::try_interpolate_for_tween_sampling_with_solver(
                points,
                options,
                maximum,
                &mut solve_cubic_system,
            )
        },
    )
}

#[cfg(test)]
mod tests;
