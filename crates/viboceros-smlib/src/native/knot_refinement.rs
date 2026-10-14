//! NLib refinement and basis editing with complete source definitions.
use super::*;
use viboceros_geometry::SurfaceKnotDirection;

/// Match section degree, knots and endpoint-weight policy using the existing
/// native clamping, exact elevation and refinement operations.
pub fn prepare_loft_sections(curves: &[NurbsCurve]) -> Result<Vec<NurbsCurve>, Error> {
    viboceros_geometry::prepare_loft_sections_with_operations(
        curves,
        &mut clamp_curve,
        &mut |curve, degree| change_curve_degree(curve, degree, false),
        &mut insert_curve_knot,
    )
}

/// Build the existing one-rail frame/blending model with native section
/// clamping, elevation and knot matching under the shared common weight gauge.
pub fn sweep_model(
    rail: viboceros_geometry::CurveRef<'_>,
    sections: &[viboceros_geometry::SweepSection],
    style: viboceros_geometry::SweepFrameStyle,
    blend: viboceros_geometry::SweepBlend,
    tolerance: Tolerance,
) -> Result<viboceros_geometry::Sweep1, Error> {
    viboceros_geometry::Sweep1::try_new_with_section_preparation(
        rail,
        sections,
        style,
        blend,
        tolerance,
        &mut |curves, maximum, limit| {
            viboceros_geometry::prepare_nurbs_sections_with_operations(
                curves,
                maximum,
                limit,
                viboceros_geometry::SectionWeightScale::Common,
                &mut clamp_curve,
                &mut |curve, degree| change_curve_degree(curve, degree, false),
                &mut insert_curve_knot,
            )
        },
    )
}

/// Refit/retained rail-basis surface through native rail fitting, clamping,
/// refinement and homogeneous section interpolation.
pub fn sweep_surface(
    sweep: &viboceros_geometry::Sweep1,
    refit: bool,
) -> Result<NurbsSurface, Error> {
    with_sweep_operations(|operations| {
        if refit {
            sweep.to_surface_with_operations(operations)
        } else {
            sweep.to_rail_basis_surface_with_operations(operations)
        }
    })
}

pub fn fit_sweep_model_surface(sweep: &viboceros_geometry::Sweep1) -> Result<NurbsSurface, Error> {
    with_sweep_operations(|operations| sweep.fit_model_surface_with_operations(operations))
}

fn with_sweep_operations(
    run: impl FnOnce(
        &mut viboceros_geometry::SweepSurfaceOperations<'_, Error>,
    ) -> Result<NurbsSurface, Error>,
) -> Result<NurbsSurface, Error> {
    let mut clamp = clamp_curve;
    let mut refine = insert_curve_knot;
    let mut fit = fit_curve;
    let mut solve = |input: &viboceros_geometry::FixedInterpolationSystem<'_>| {
        solve_with_fixed_rows(&input.system, Some(input.fixed_rows))
    };
    run(&mut viboceros_geometry::SweepSurfaceOperations {
        clamp: &mut clamp,
        refine: &mut refine,
        fit: &mut fit,
        solve: &mut solve,
    })
}

/// Ordered Loft surface with native common-basis preparation and the shared
/// section spacing, homogeneous interpolation and endpoint-weight policies.
pub fn loft_surface(
    curves: &[NurbsCurve],
    style: viboceros_geometry::LoftStyle,
    closed: bool,
) -> Result<NurbsSurface, Error> {
    viboceros_geometry::try_loft_nurbs_curves_with_operations(
        curves,
        style,
        closed,
        &mut prepare_loft_sections,
        &mut solve_cubic_system,
    )
}

#[repr(C)]
struct InterpolationSystem {
    knots: *const f64,
    parameters: *const f64,
    targets: *const f64,
    knot_count: usize,
    parameter_count: usize,
    columns: usize,
    target_count: usize,
    degree: usize,
    closed: c_int,
    handles: c_int,
    uniform_periodic: c_int,
    fixed_rows: *const u8,
    fixed_count: usize,
    point_grid: c_int,
    amplification: *mut f64,
    amplification_count: usize,
    nonuniform_periodic: c_int,
    periodic_control_shift: usize,
    derivative_orders: *const u8,
    derivative_count: usize,
}

fn solve_cubic_system(
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

fn solve_rebuild_interpolation(
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

fn solve_with_fixed_rows(
    input: &viboceros_geometry::NurbsInterpolationSystem<'_>,
    fixed: Option<&[u8]>,
) -> Result<Vec<f64>, Error> {
    solve_interpolation(input, fixed, None)
}

fn solve_interpolation(
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

/// Interpolate an open mean-chord point grid using the existing native basis
/// and batched banded solve; degree/count and residual policies stay shared.
pub fn interpolate_open_point_grid(
    points: &[Point3],
    count: [usize; 2],
    degree: [usize; 2],
) -> Result<NurbsSurface, Error> {
    interpolate_point_grid(points, count, degree, [false; 2])
}

/// Open or closed Rhino-style point-grid interpolation through native basis
/// continuation and batched LU; core knot/topology/residual policy stays shared.
pub fn interpolate_point_grid(
    points: &[Point3],
    count: [usize; 2],
    degree: [usize; 2],
    closed: [bool; 2],
) -> Result<NurbsSurface, Error> {
    NurbsSurface::try_through_point_grid_with_solver(
        points,
        count,
        degree,
        closed,
        &mut solve_point_grid,
    )
}

pub fn point_grid_brep(
    points: &[Point3],
    count: [usize; 2],
    degree: [usize; 2],
    closed: [bool; 2],
    tolerance: Tolerance,
) -> Result<viboceros_geometry::Brep, Error> {
    viboceros_geometry::Brep::try_through_point_grid_with_solver(
        points,
        count,
        degree,
        closed,
        tolerance,
        &mut solve_point_grid,
    )
}

fn solve_point_grid(
    input: &viboceros_geometry::PointGridInterpolationSystem<'_>,
) -> Result<viboceros_geometry::PointGridInterpolationSolution, Error> {
    let mut amplification = vec![0.; input.parameters.len()];
    let view = InterpolationSystem {
        knots: input.knots.as_ptr(),
        parameters: input.parameters.as_ptr(),
        targets: input.targets.as_ptr(),
        knot_count: input.knots.len(),
        parameter_count: input.parameters.len(),
        columns: input.columns,
        target_count: input.targets.len(),
        degree: input.degree,
        closed: c_int::from(input.closed),
        handles: 0,
        uniform_periodic: 0,
        fixed_rows: std::ptr::null(),
        fixed_count: 0,
        point_grid: 1,
        amplification: amplification.as_mut_ptr(),
        amplification_count: amplification.len(),
        nonuniform_periodic: 0,
        periodic_control_shift: 0,
        derivative_orders: std::ptr::null(),
        derivative_count: 0,
    };
    let mut controls = vec![0.; input.targets.len()];
    call(|error, capacity| unsafe {
        vb_interpolation_solve(
            &view,
            controls.as_mut_ptr(),
            controls.len(),
            error,
            capacity,
        )
    })?;
    Ok(viboceros_geometry::PointGridInterpolationSolution {
        controls,
        amplification,
    })
}

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

unsafe extern "C" {
    fn vb_interpolation_solve(
        input: *const InterpolationSystem,
        output: *mut f64,
        output_count: usize,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_nurbs_surface_isocurve(
        source: *const brep::NurbsData,
        varying: c_int,
        parameter: f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_nurbs_trim_surface_axis(
        source: *const brep::NurbsData,
        axis: c_int,
        start: f64,
        end: f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_nurbs_trim_curve(
        source: *const brep::NurbsData,
        start: f64,
        end: f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_nurbs_bezier(
        source: *const brep::NurbsData,
        direction: c_int,
        out: *mut *mut brep::NativeSurfaceData,
        count: usize,
        control_error: *mut c_int,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_nurbs_fit_basis(
        source: *const brep::NurbsData,
        axis: c_int,
        knots: *const f64,
        knot_count: usize,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_nurbs_change_degree(
        source: *const brep::NurbsData,
        axis: c_int,
        degree: usize,
        deformable: c_int,
        knots: *const f64,
        knot_count: usize,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_nurbs_insert_knot(
        source: *const brep::NurbsData,
        axis: c_int,
        parameter: f64,
        target: usize,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Extract an exact isocurve with the complete varying-axis basis and gauge.
pub fn surface_isocurve(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: f64,
) -> Result<NurbsCurve, Error> {
    source.isocurve_with_extractor(
        direction,
        parameter,
        &mut |surface, direction, parameter| {
            let input = NurbsInput::surface(surface);
            let view = input.view();
            let mut pointer = std::ptr::null_mut();
            let varying = if direction == SurfaceKnotDirection::U {
                0
            } else {
                1
            };
            call(|error, capacity| unsafe {
                vb_nurbs_surface_isocurve(&view, varying, parameter, &mut pointer, error, capacity)
            })?;
            brep::curve_from_native(pointer)
        },
    )
}

/// Trim an exact parameter interval, retaining the original chart and gauge.
pub fn trim_curve(
    source: &NurbsCurve,
    interval: std::ops::RangeInclusive<f64>,
) -> Result<NurbsCurve, Error> {
    viboceros_geometry::validate_curve_trim_interval(&interval, source.domain())?;
    // The shared exact affine path preserves zero crossings needed by
    // intersection topology; a native floating ratio can round them away.
    if interval == source.domain()
        || (source.degree() == 1
            && source.control_points().len() == 2
            && source.control_points()[0].weight() == source.control_points()[1].weight())
    {
        return Ok(source.try_trimmed(interval)?);
    }
    let input = NurbsInput::curve(source);
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_trim_curve(
            &view,
            *interval.start(),
            *interval.end(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    brep::curve_from_native(pointer)
}

/// Extract a directed NURBS subcurve, reusing the shared reversal/seam policy.
pub fn subcurve(source: &NurbsCurve, start: f64, end: f64) -> Result<NurbsCurve, Error> {
    source.try_subcurve_with_extractor(start, end, &mut trim_curve)
}

/// Anchor-relative signed-length extraction using the shared sampler and
/// native NURBS trimming, retaining analytic and composite leaf classes.
pub fn subcurve_at_arc_length(
    source: &viboceros_geometry::Curve3,
    anchor: f64,
    length: f64,
    tolerance: Tolerance,
) -> Result<Option<(viboceros_geometry::Curve3, f64)>, Error> {
    source.try_subcurve_at_arc_length_with_operations(
        anchor,
        length,
        tolerance,
        &mut trim_curve,
        &mut change_curve_seam,
    )
}

/// Relocate a closed NURBS seam using native refinement or checked segment
/// extraction, with the shared cyclic control/knot and homogeneous join policy.
pub fn change_curve_seam(source: &NurbsCurve, parameter: f64) -> Result<NurbsCurve, Error> {
    source.try_change_closed_seam_with_operations(
        parameter,
        &mut insert_curve_knot,
        &mut |curve, parameter, _periodic| split_curve(curve, parameter),
    )
}

/// Relocate closed tensor seams in U then V using native whole-net refinement
/// or checked directional extraction and shared homogeneous seam assembly.
pub fn change_surface_seam(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter_u: f64,
    parameter_v: f64,
) -> Result<NurbsSurface, Error> {
    source.try_change_closed_seam_with_operations(
        direction,
        parameter_u,
        parameter_v,
        &mut insert_surface_knot,
        &mut |surface, axis, parameter, _periodic| {
            let domain = match axis {
                SurfaceKnotDirection::U => surface.domain_u(),
                SurfaceKnotDirection::V => surface.domain_v(),
                SurfaceKnotDirection::Both => unreachable!("one seam direction at a time"),
            };
            Ok((
                trim_surface_direction(surface, axis, *domain.start()..=parameter)?,
                trim_surface_direction(surface, axis, parameter..=*domain.end())?,
            ))
        },
    )
}

/// Restrict one surface axis, retaining the complete unselected basis and
/// exact whole-domain no-ops. Two-control lines reuse the shared exact affine
/// path; rational lines and other control curves use native extraction.
pub fn trim_surface_direction(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    interval: std::ops::RangeInclusive<f64>,
) -> Result<NurbsSurface, Error> {
    source.try_trimmed_direction_with_extractor(
        direction,
        interval,
        &mut |surface, direction, range| {
            let (degree, width) = match direction {
                SurfaceKnotDirection::U => (surface.degree_u(), surface.control_point_count_u()),
                SurfaceKnotDirection::V => (surface.degree_v(), surface.control_point_count_v()),
                SurfaceKnotDirection::Both => unreachable!("direction was validated"),
            };
            if degree == 1 && width == 2 {
                surface.try_trimmed_direction_with_nurbs(direction, range, &mut trim_curve)
            } else {
                trim_surface_axis(surface, direction, *range.start(), *range.end())
            }
        },
    )
}

/// Extract a rectangular subdomain in U then V with native directional strips.
pub fn trim_surface(
    source: &NurbsSurface,
    u: std::ops::RangeInclusive<f64>,
    v: std::ops::RangeInclusive<f64>,
) -> Result<NurbsSurface, Error> {
    source.try_trimmed_with_extractor(u, v, &mut trim_surface_direction)
}

/// Split one surface axis into two clamped intervals using checked extraction.
pub fn split_surface(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: f64,
) -> Result<(NurbsSurface, NurbsSurface), Error> {
    source.try_split_direction_with_extractor(
        direction,
        parameter,
        &mut |surface, direction, parameter| {
            let domain = match direction {
                SurfaceKnotDirection::U => surface.domain_u(),
                SurfaceKnotDirection::V => surface.domain_v(),
                SurfaceKnotDirection::Both => unreachable!("direction was validated"),
            };
            Ok((
                trim_surface_direction(surface, direction, *domain.start()..=parameter)?,
                trim_surface_direction(surface, direction, parameter..=*domain.end())?,
            ))
        },
    )
}

fn trim_surface_axis(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    start: f64,
    end: f64,
) -> Result<NurbsSurface, Error> {
    let axis = match direction {
        SurfaceKnotDirection::U => 0,
        SurfaceKnotDirection::V => 1,
        SurfaceKnotDirection::Both => unreachable!("one seam direction at a time"),
    };
    let input = NurbsInput::surface(source);
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_trim_surface_axis(&view, axis, start, end, &mut pointer, error, capacity)
    })?;
    brep::surface_from_native(pointer)
}

/// Split once into the two clamped parameter intervals with native extraction.
pub fn split_curve(source: &NurbsCurve, parameter: f64) -> Result<(NurbsCurve, NurbsCurve), Error> {
    if !parameter.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "NURBS curve split parameter",
        }
        .into());
    }
    let domain = source.domain();
    if parameter <= *domain.start() || parameter >= *domain.end() {
        return Err(GeometryError::InvalidCurveSplitParameter.into());
    }
    Ok((
        trim_curve(source, *domain.start()..=parameter)?,
        trim_curve(source, parameter..=*domain.end())?,
    ))
}

/// Split at sorted distinct interior parameters, retaining closed cyclic order.
pub fn split_curve_at_parameters(
    source: &NurbsCurve,
    parameters: &[f64],
) -> Result<Vec<NurbsCurve>, Error> {
    source.try_split_at_parameters_with_extractor(parameters, &mut trim_curve)
}

fn decompose(
    input: NurbsInput<'_>,
    direction: c_int,
    count: usize,
    output_controls: usize,
) -> Result<Vec<*mut brep::NativeSurfaceData>, Error> {
    if output_controls > viboceros_geometry::MAX_BEZIER_CONTROL_POINTS {
        return Err(GeometryError::BezierDecompositionLimit.into());
    }
    let view = input.view();
    let mut pointers = vec![std::ptr::null_mut(); count];
    let mut control_error = 0;
    let result = call(|error, capacity| unsafe {
        vb_nurbs_bezier(
            &view,
            direction,
            pointers.as_mut_ptr(),
            count,
            &mut control_error,
            error,
            capacity,
        )
    });
    if control_error != 0 {
        return Err(GeometryError::UnrepresentableBezierControl.into());
    }
    result?;
    Ok(pointers)
}

/// Extract exact clamped Bezier curves with the original active span domains.
pub fn curve_bezier_spans(source: &NurbsCurve) -> Result<Vec<NurbsCurve>, Error> {
    let count = source.spans().count();
    let controls = count.saturating_mul(source.degree().saturating_add(1));
    brep::curves_from_native_batch(decompose(NurbsInput::curve(source), 0, count, controls)?)
}

/// Extract single-axis strips or U-outer/V-inner tensor Bezier patches.
/// The unselected direction retains its complete source basis.
pub fn surface_single_span_patches(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
) -> Result<Vec<NurbsSurface>, Error> {
    let u = direction != SurfaceKnotDirection::V;
    let v = direction != SurfaceKnotDirection::U;
    let count = (if u { source.spans_u().count() } else { 1 }).saturating_mul(if v {
        source.spans_v().count()
    } else {
        1
    });
    let controls = count
        .saturating_mul(if u {
            source.degree_u().saturating_add(1)
        } else {
            source.control_point_count_u()
        })
        .saturating_mul(if v {
            source.degree_v().saturating_add(1)
        } else {
            source.control_point_count_v()
        });
    brep::surfaces_from_native_batch(decompose(
        NurbsInput::surface(source),
        match direction {
            SurfaceKnotDirection::U => 0,
            SurfaceKnotDirection::V => 1,
            SurfaceKnotDirection::Both => 2,
        },
        count,
        controls,
    )?)
}

fn clamp_curve(source: &NurbsCurve) -> Result<NurbsCurve, Error> {
    let domain = source.domain();
    let target = source.degree() + 1;
    let result = insert_curve_knot(source, *domain.start(), target)?;
    insert_curve_knot(&result, *domain.end(), target)
}

pub(super) fn clamp_surface_direction(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
) -> Result<NurbsSurface, Error> {
    let (domain, degree) = match direction {
        SurfaceKnotDirection::U => (source.domain_u(), source.degree_u()),
        SurfaceKnotDirection::V => (source.domain_v(), source.degree_v()),
        SurfaceKnotDirection::Both => {
            return Err(Error::Kernel(
                "Basis editing requires one parameter direction".into(),
            ));
        }
    };
    let result = insert_surface_knot(source, direction, *domain.start(), degree + 1)?;
    insert_surface_knot(&result, direction, *domain.end(), degree + 1)
}

fn fit_basis(
    input: NurbsInput<'_>,
    axis: c_int,
    knots: &[f64],
) -> Result<*mut brep::NativeSurfaceData, Error> {
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_fit_basis(
            &view,
            axis,
            knots.as_ptr(),
            knots.len(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    Ok(pointer)
}

/// Remove the knot chosen by the existing model-space command policy, fitting
/// the replacement homogeneous controls with the shared native basis editor.
pub fn remove_curve_knot(source: &NurbsCurve, parameter: f64) -> Result<NurbsCurve, Error> {
    let knot = source.knot_removal_candidate(parameter)?;
    let clamped = clamp_curve(source)?;
    let knots = viboceros_geometry::knot_removal_knots(clamped.degree(), clamped.knots(), knot)?;
    brep::curve_from_native(fit_basis(NurbsInput::curve(&clamped), 0, &knots)?)
}

/// Remove one knot in the requested tensor direction. Selection uses parameter
/// distance and the complete net's periodic state, rather than an individual row.
pub fn remove_surface_knot(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: f64,
) -> Result<NurbsSurface, Error> {
    let knot = source.knot_removal_candidate(direction, parameter)?;
    let clamped = clamp_surface_direction(source, direction)?;
    let (axis, degree, before) = match direction {
        SurfaceKnotDirection::U => (0, clamped.degree_u(), clamped.knots_u()),
        SurfaceKnotDirection::V => (1, clamped.degree_v(), clamped.knots_v()),
        SurfaceKnotDirection::Both => unreachable!("candidate requires one parameter direction"),
    };
    let knots = viboceros_geometry::knot_removal_knots(degree, before, knot)?;
    brep::surface_from_native(fit_basis(NurbsInput::surface(&clamped), axis, &knots)?)
}

/// Collapse the command-selected groups from higher to lower parameters,
/// fitting each complete group once with the shared native interpolator.
pub fn remove_curve_multiple_knots(
    source: &NurbsCurve,
    remove_fully_multiple_knots: bool,
    maximum_kink_angle_radians: f64,
) -> Result<(NurbsCurve, usize), Error> {
    let removals = source.multiple_knot_removal_candidates(
        remove_fully_multiple_knots,
        maximum_kink_angle_radians,
    )?;
    let removed = removals.iter().map(|(_, count)| *count).sum();
    if removals.is_empty() {
        return Ok((source.clone(), 0));
    }
    let mut result = clamp_curve(source)?;
    for (knot, count) in removals.iter().rev() {
        let knots = viboceros_geometry::knot_group_removal_knots(
            result.degree(),
            result.knots(),
            *knot,
            *count,
        )?;
        result = brep::curve_from_native(fit_basis(NurbsInput::curve(&result), 0, &knots)?)?;
    }
    Ok((result, removed))
}

/// Collapse selected U groups, then recompute and collapse V groups on the
/// edited tensor. Validate the original periodic flags before either edit.
pub fn remove_surface_multiple_knots(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    remove_fully_multiple_knots: bool,
    maximum_kink_angle_radians: f64,
) -> Result<(NurbsSurface, usize), Error> {
    source.validate_multiple_knot_removal(direction, maximum_kink_angle_radians)?;
    let mut result = source.clone();
    let mut removed = 0;
    for (axis, selected) in [
        (
            SurfaceKnotDirection::U,
            matches!(
                direction,
                SurfaceKnotDirection::U | SurfaceKnotDirection::Both
            ),
        ),
        (
            SurfaceKnotDirection::V,
            matches!(
                direction,
                SurfaceKnotDirection::V | SurfaceKnotDirection::Both
            ),
        ),
    ] {
        if !selected {
            continue;
        }
        let removals = result.multiple_knot_removal_candidates(
            axis,
            remove_fully_multiple_knots,
            maximum_kink_angle_radians,
        )?;
        if removals.is_empty() {
            continue;
        }
        result = clamp_surface_direction(&result, axis)?;
        for (knot, count) in removals.iter().rev() {
            let (native_axis, degree, before) = match axis {
                SurfaceKnotDirection::U => (0, result.degree_u(), result.knots_u()),
                SurfaceKnotDirection::V => (1, result.degree_v(), result.knots_v()),
                SurfaceKnotDirection::Both => unreachable!("stages choose one parameter axis"),
            };
            let knots =
                viboceros_geometry::knot_group_removal_knots(degree, before, *knot, *count)?;
            result = brep::surface_from_native(fit_basis(
                NurbsInput::surface(&result),
                native_axis,
                &knots,
            )?)?;
            removed += count;
        }
    }
    Ok((result, removed))
}

pub(super) struct NurbsInput<'a> {
    degree: [usize; 2],
    count: [usize; 2],
    knots: [&'a [f64]; 2],
    points: Vec<f64>,
}
impl<'a> NurbsInput<'a> {
    fn new(
        degree: [usize; 2],
        count: [usize; 2],
        knots: [&'a [f64]; 2],
        controls: &[WeightedPoint3],
    ) -> Self {
        Self {
            degree,
            count,
            knots,
            points: controls
                .iter()
                .flat_map(|p| {
                    let [x, y, z] = p.point().to_array();
                    [x, y, z, p.weight()]
                })
                .collect(),
        }
    }
    pub(super) fn curve(source: &'a NurbsCurve) -> Self {
        Self::new(
            [source.degree(), 0],
            [source.control_points().len(), 1],
            [source.knots(), &[]],
            source.control_points(),
        )
    }
    fn surface(source: &'a NurbsSurface) -> Self {
        Self::new(
            [source.degree_u(), source.degree_v()],
            [
                source.control_point_count_u(),
                source.control_point_count_v(),
            ],
            [source.knots_u(), source.knots_v()],
            source.control_points(),
        )
    }
    pub(super) fn view(&self) -> brep::NurbsData {
        brep::NurbsData {
            degree: self.degree,
            count: self.count,
            knot_count: self.knots.map(|k| k.len()),
            knots: self.knots.map(|k| k.as_ptr()),
            points: self.points.as_ptr(),
        }
    }
}

/// Native exact elevation or homogeneous interpolation in the existing
/// application target basis. Degree changes clamp the active source interval.
pub fn change_curve_degree(
    source: &NurbsCurve,
    degree: usize,
    deformable: bool,
) -> Result<NurbsCurve, Error> {
    if degree == 0 {
        return Err(GeometryError::InvalidDegree.into());
    }
    if degree == source.degree() {
        return Ok(source.clone());
    }
    let clamped = clamp_curve(source)?;
    let knots = viboceros_geometry::degree_change_knots(
        clamped.degree(),
        degree,
        deformable,
        clamped.knots(),
    )?;
    let input = NurbsInput::curve(&clamped);
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_change_degree(
            &view,
            0,
            degree,
            c_int::from(deformable),
            knots.as_ptr(),
            knots.len(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    brep::curve_from_native(pointer)
}

pub fn change_surface_degree(
    source: &NurbsSurface,
    u: usize,
    v: usize,
    deformable: bool,
) -> Result<NurbsSurface, Error> {
    if u == 0 || v == 0 {
        return Err(GeometryError::InvalidDegree.into());
    }
    let mut result = source.clone();
    for (axis, degree) in [(0, u), (1, v)] {
        let old = if axis == 0 {
            result.degree_u()
        } else {
            result.degree_v()
        };
        if old == degree {
            continue;
        }
        let direction = if axis == 0 {
            SurfaceKnotDirection::U
        } else {
            SurfaceKnotDirection::V
        };
        result = clamp_surface_direction(&result, direction)?;
        let before = if axis == 0 {
            result.knots_u()
        } else {
            result.knots_v()
        };
        let knots = viboceros_geometry::degree_change_knots(old, degree, deformable, before)?;
        let input = NurbsInput::surface(&result);
        let view = input.view();
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_nurbs_change_degree(
                &view,
                axis,
                degree,
                c_int::from(deformable),
                knots.as_ptr(),
                knots.len(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        result = brep::surface_from_native(pointer)?;
    }
    Ok(result)
}

/// Clamp a periodic curve at both active endpoints using the shared native
/// split path. Non-periodic definitions are returned unchanged.
pub fn make_curve_non_periodic(source: &NurbsCurve) -> Result<NurbsCurve, Error> {
    if !source.is_periodic() {
        return Ok(source.clone());
    }
    clamp_curve(source)
}

/// Clamp only periodic tensor directions selected by the caller. Determine
/// both flags before editing either axis, matching the portable policy.
pub fn make_surface_non_periodic(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
) -> Result<NurbsSurface, Error> {
    let clamp_u = matches!(
        direction,
        SurfaceKnotDirection::U | SurfaceKnotDirection::Both
    ) && source.is_periodic_u();
    let clamp_v = matches!(
        direction,
        SurfaceKnotDirection::V | SurfaceKnotDirection::Both
    ) && source.is_periodic_v();
    let mut result = source.clone();
    for (axis, clamp) in [
        (SurfaceKnotDirection::U, clamp_u),
        (SurfaceKnotDirection::V, clamp_v),
    ] {
        if clamp {
            result = clamp_surface_direction(&result, axis)?;
        }
    }
    Ok(result)
}

pub fn insert_curve_knot(
    source: &NurbsCurve,
    parameter: f64,
    target: usize,
) -> Result<NurbsCurve, Error> {
    Ok(source.restore_knot_insertion_topology(
        refine_curve_payload(source, parameter, target)?,
        parameter,
        target,
    )?)
}

/// Modeling preparation retains the complete outer padding; the InsertKnot
/// command's OpenNURBS outer-knot convention is applied only by its public edit.
pub(super) fn clamp_curve_endpoint(
    source: &NurbsCurve,
    at_start: bool,
) -> Result<NurbsCurve, Error> {
    let domain = source.domain();
    let parameter = if at_start {
        *domain.start()
    } else {
        *domain.end()
    };
    refine_curve_payload(source, parameter, source.degree() + 1)
}

fn refine_curve_payload(
    source: &NurbsCurve,
    parameter: f64,
    target: usize,
) -> Result<NurbsCurve, Error> {
    let input = NurbsInput::curve(source);
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_insert_knot(&view, 0, parameter, target, &mut pointer, error, capacity)
    })?;
    brep::curve_from_native(pointer)
}

pub fn insert_surface_knot(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: f64,
    target: usize,
) -> Result<NurbsSurface, Error> {
    let axis = match direction {
        SurfaceKnotDirection::U => 0,
        SurfaceKnotDirection::V => 1,
        SurfaceKnotDirection::Both => {
            return Err(Error::Kernel(
                "Knot refinement requires one parameter direction".into(),
            ));
        }
    };
    let input = NurbsInput::surface(source);
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_insert_knot(
            &view,
            axis,
            parameter,
            target,
            &mut pointer,
            error,
            capacity,
        )
    })?;
    Ok(source.restore_knot_insertion_topology(
        brep::surface_from_native(pointer)?,
        direction,
        parameter,
        target,
    )?)
}

#[cfg(test)]
mod interpolation_tests {
    use super::*;

    #[test]
    fn native_blend_hermite_rows_reproduce_independent_monomials_and_tiny_channels() {
        use viboceros_geometry::{SurfaceBlendContinuity as G, SurfaceBlendInterpolation};
        for first in [G::Position, G::Tangency, G::Curvature] {
            for second in [G::Position, G::Tangency, G::Curvature] {
                let p = first.order() + second.order() + 1;
                for count in [1, 513, 4096] {
                    let scales = [0., 1e-280, 1., 1e280];
                    let mut targets = Vec::new();
                    for (end, order) in (0..=first.order())
                        .map(|order| (false, order))
                        .chain((0..=second.order()).rev().map(|order| (true, order)))
                    {
                        // Polynomial channels contain v^p and linear functions.
                        // Their Bernstein controls follow linear precision and
                        // v^p=B[p,p]; no portable Hermite constructor is called.
                        let monomial = match order {
                            0 => f64::from(end),
                            1 if end || p == 1 => p as f64,
                            2 if end || p == 2 => (p * (p - 1)) as f64,
                            _ => 0.,
                        };
                        let linear = match order {
                            0 => f64::from(end),
                            1 => 1.,
                            _ => 0.,
                        };
                        let constant = f64::from(order == 0);
                        for u in 0..count {
                            let scale = scales[u % scales.len()];
                            targets.push([
                                scale * monomial,
                                scale * (2. * constant + 3. * linear + monomial),
                                -scale * (linear + monomial),
                                constant + 0.25 * linear + 0.125 * monomial,
                            ]);
                        }
                    }
                    let output = interpolate_surface_blend(&SurfaceBlendInterpolation {
                        continuity: [first, second],
                        edge_control_count: count,
                        targets: &targets,
                    })
                    .unwrap();
                    assert_eq!(output.len(), (p + 1) * count);
                    for row in 0..=p {
                        for u in 0..count {
                            let scale = scales[u % scales.len()];
                            let t = row as f64 / p as f64;
                            let monomial = f64::from(row == p);
                            let expected = [
                                scale * monomial,
                                scale * (2. + 3. * t + monomial),
                                -scale * (t + monomial),
                                1. + 0.25 * t + 0.125 * monomial,
                            ];
                            for k in 0..4 {
                                let magnitude = if k == 3 { 1. } else { scale };
                                assert!(
                                    (output[row * count + u][k] - expected[k]).abs()
                                        <= 1e-12 * magnitude,
                                    "{first:?}/{second:?} count={count} row={row} u={u} k={k}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn equal_station_rows_require_independent_fixed_constraints() {
        // Two separated cubic Bezier intervals represent X=6t and X=7+6t.
        // Linear precision gives independent controls [0,1,2,3,10,11,12,13].
        let knots = [0., 0., 0., 0., 0.5, 0.5, 0.5, 0.5, 1., 1., 1., 1.];
        let parameters = [0., 1. / 6., 1. / 3., 0.5, 0.5, 2. / 3., 5. / 6., 1.];
        let targets = [0., 1., 2., 3., 10., 11., 12., 13.]
            .into_iter()
            .flat_map(|x| [x, 0., 0., 1.])
            .collect::<Vec<_>>();
        let system = viboceros_geometry::NurbsInterpolationSystem {
            degree: 3,
            knots: &knots,
            parameters: &parameters,
            periodic: false,
            columns: 4,
            targets: &targets,
        };
        let mask = [1, 0, 0, 1, 1, 0, 0, 1];
        let output = solve_with_fixed_rows(&system, Some(&mask)).unwrap();
        for (a, b) in output.iter().zip(&targets) {
            assert!((a - b).abs() < 1e-12);
        }
        let mut invalid = mask;
        invalid[4] = 0;
        assert!(matches!(
            solve_with_fixed_rows(&system, Some(&invalid)),
            Err(Error::Kernel(_))
        ));
        assert_eq!(system.targets, targets);
    }
    #[test]
    fn closed_grid_band_retries_native_pivoting_for_an_invertible_zero_diagonal() {
        // Ordered integer stations shifted by one give the exact cyclic
        // permutation A[i,(i+1)%64]=1. Every diagonal entry is zero, so native
        // unpivoted band LU must reject this invertible system. The existing
        // native pivoted producer must recover every tiny and ordinary channel.
        let n = 64;
        let knots = [
            vec![0., 0.],
            (1..n).map(|i| i as f64).collect(),
            vec![n as f64; n.min(2)],
        ]
        .concat();
        let parameters = (1..=n).map(|i| i as f64).collect::<Vec<_>>();
        let expected = (0..n)
            .flat_map(|i| [i as f64, -(i as f64), (i + 1) as f64 * 1e-280, 1.])
            .collect::<Vec<_>>();
        let targets = (0..n)
            .flat_map(|i| {
                expected[((i + 1) % n) * 4..((i + 1) % n) * 4 + 4]
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        let job = viboceros_geometry::PointGridInterpolationSystem {
            degree: 1,
            knots: &knots,
            parameters: &parameters,
            closed: true,
            columns: 4,
            targets: &targets,
        };
        let output = solve_point_grid(&job).unwrap();
        assert_eq!(output.controls, expected);
        assert_eq!(output.amplification, vec![1.; n]);
        assert_eq!(job.targets, targets);
    }

    #[test]
    fn nonuniform_periodic_band_retries_pivoting_and_accepts_a_final_endpoint_site() {
        // A one-station phase shift gives an independently invertible cyclic
        // permutation with every diagonal zero. The last actual site is the
        // domain end; the appended interval endpoint is metadata, not a row.
        let n = 64;
        let knots = [
            vec![0., 0.],
            (1..n).map(|i| i as f64).collect(),
            vec![n as f64; 2],
        ]
        .concat();
        let parameters = [
            (1..=n).map(|i| i as f64).collect::<Vec<_>>(),
            vec![n as f64],
        ]
        .concat();
        let expected = (0..n)
            .flat_map(|i| [i as f64, -(i as f64), (i + 1) as f64 * 1e-280, 1.])
            .collect::<Vec<_>>();
        let targets = (0..n)
            .flat_map(|i| {
                expected[((i + 1) % n) * 4..((i + 1) % n) * 4 + 4]
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        let job = viboceros_geometry::NurbsInterpolationSystem {
            degree: 1,
            knots: &knots,
            parameters: &parameters,
            periodic: true,
            columns: 4,
            targets: &targets,
        };
        assert_eq!(solve_interpolation(&job, None, Some(0)).unwrap(), expected);
        assert_eq!(job.targets, targets);
        assert!(solve_interpolation(&job, None, None).is_err());
    }
}
