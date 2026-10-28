//! NLib refinement and basis editing with complete source definitions.
use super::*;
use viboceros_geometry::SurfaceKnotDirection;

mod interpolation;
mod loft;
mod trim;

pub use interpolation::{
    average_match_curve_ends, curve_edit_point_interpolation, curve_tweens,
    fit_cubic_curve_at_stations, fit_cubic_curve_with_constraints, fit_curve,
    interpolate_affine_greville, interpolate_curve, interpolate_curve_for_command,
    interpolate_endpoints, interpolate_open_point_grid, interpolate_point_grid,
    interpolate_surface_blend, make_curve_periodic, make_surface_periodic, match_curve_end,
    morph_interpolation, point_grid_brep, rebuild_curve, rebuild_surface, surface_tweens,
    surface_tweens_refitted, surface_tweens_sampled,
};
use interpolation::{
    solve_cubic_system, solve_interpolation, solve_rebuild_interpolation, solve_with_fixed_rows,
};
pub use loft::{
    fit_sweep_model_surface, loft_surface, loft_surface_rebuilt, loft_surface_refitted,
    loft_surface_refitted_with_angle, loft_surface_with_end_points, prepare_loft_sections,
    sweep_model, sweep_model_with_point_ends, sweep_surface,
};
pub use trim::{
    change_curve_seam, change_surface_seam, curve_bezier_spans, split_curve,
    split_curve_at_parameters, split_surface, subcurve, subcurve_at_arc_length, surface_isocurve,
    surface_single_span_patches, trim_curve, trim_surface, trim_surface_direction,
};

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

/// Remove an explicitly selected marker, avoiding reselection at self intersections.
pub fn remove_curve_knot_at_value(source: &NurbsCurve, knot: f64) -> Result<NurbsCurve, Error> {
    if source.is_periodic() {
        return source.try_remove_periodic_knot_value_with_solver(knot, &mut |job, shift| {
            solve_interpolation(job, None, Some(shift))
        });
    }
    let clamped = clamp_curve(source)?;
    let knots = viboceros_geometry::knot_removal_knots(clamped.degree(), clamped.knots(), knot)?;
    brep::curve_from_native(fit_basis(NurbsInput::curve(&clamped), 0, &knots)?)
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

/// Share multiplicity selection and complete displacement checks with the
/// portable kernel, proposing controls through native basis interpolation.
pub fn reduce_curve_knots_in_range(
    source: &NurbsCurve,
    minimum: usize,
    maximum: usize,
    limit: Option<f64>,
) -> Result<viboceros_geometry::KnotReduction, Error> {
    source.try_reduce_knots_in_range_with_remover(minimum, maximum, limit, &mut |curve, knots| {
        brep::curve_from_native(fit_basis(NurbsInput::curve(curve), 0, knots)?)
    })
}

/// Collapse multiple groups using the command's angular eligibility policy.
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
    pub(super) fn surface(source: &'a NurbsSurface) -> Self {
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

pub fn remove_periodic_surface_knot(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: f64,
) -> Result<NurbsSurface, Error> {
    source.try_remove_periodic_knot_with_solver(direction, parameter, &mut |job, shift| {
        solve_interpolation(job, None, Some(shift))
    })
}
