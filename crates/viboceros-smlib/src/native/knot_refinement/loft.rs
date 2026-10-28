//! Native Loft and Sweep section preparation.
use super::*;

/// Share section-region parameters, then match degree, knots and endpoint
/// weights with existing native clamping, exact elevation and refinement.
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
        &mut prepare_sweep_sections,
    )
}

/// Prepare real sections natively before constructing exact point-pole rows.
pub fn sweep_model_with_point_ends(
    rail: viboceros_geometry::CurveRef<'_>,
    sections: &[viboceros_geometry::SweepSection],
    ends: viboceros_geometry::SweepEndPoints,
    style: viboceros_geometry::SweepFrameStyle,
    blend: viboceros_geometry::SweepBlend,
    tolerance: Tolerance,
) -> Result<viboceros_geometry::Sweep1, Error> {
    viboceros_geometry::Sweep1::try_new_with_point_ends_and_section_preparation(
        rail,
        sections,
        ends,
        style,
        blend,
        tolerance,
        &mut prepare_sweep_sections,
    )
}

fn prepare_sweep_sections(
    curves: &[NurbsCurve],
    maximum: usize,
    limit: viboceros_geometry::GeometryError,
) -> Result<Vec<NurbsCurve>, Error> {
    viboceros_geometry::prepare_nurbs_sections_with_operations(
        curves,
        maximum,
        limit,
        viboceros_geometry::SectionWeightScale::Common,
        &mut clamp_curve,
        &mut |curve, degree| change_curve_degree(curve, degree, false),
        &mut insert_curve_knot,
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

/// Ordered point-ended Loft using native exact basis edits and cubic solving.
pub fn loft_surface_with_end_points(
    curves: &[NurbsCurve],
    style: viboceros_geometry::LoftStyle,
    ends: viboceros_geometry::LoftEndPoints,
) -> Result<NurbsSurface, Error> {
    viboceros_geometry::try_loft_nurbs_curves_with_end_points_and_operations(
        curves,
        style,
        ends,
        &mut prepare_loft_sections,
        &mut solve_cubic_system,
    )
}

/// Rebuild ordered sections using native basis evaluation and solving, then
/// reuse the ordinary native Loft preparation/interpolation and pole policy.
pub fn loft_surface_rebuilt(
    curves: &[NurbsCurve],
    style: viboceros_geometry::LoftStyle,
    closed: bool,
    ends: viboceros_geometry::LoftEndPoints,
    point_count: usize,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    let sections = viboceros_geometry::try_prepare_rebuilt_loft_sections_with(
        curves,
        style,
        closed,
        ends,
        point_count,
        tolerance,
        &mut |c, n, d, t| {
            viboceros_geometry::try_rebuild_open_curve_with_interpolation(
                viboceros_geometry::CurveRef::NurbsCurve(c),
                n,
                d,
                t,
                &mut solve_rebuild_interpolation,
            )
        },
    )?;
    if ends.is_empty() {
        loft_surface(&sections, style, closed)
    } else {
        loft_surface_with_end_points(&sections, style, ends)
    }
}

/// Refit supported ordered profiles, then use native common-basis operations
/// and interpolation, retaining certified smooth proposals where available.
pub fn loft_surface_refitted(
    curves: &[NurbsCurve],
    style: viboceros_geometry::LoftStyle,
    closed: bool,
    ends: viboceros_geometry::LoftEndPoints,
    fit_tolerance: f64,
) -> Result<NurbsSurface, Error> {
    refitted(curves, style, closed, ends, fit_tolerance, None)
}

/// Refit with an explicit corner angle in radians and native basis operations.
pub fn loft_surface_refitted_with_angle(
    curves: &[NurbsCurve],
    style: viboceros_geometry::LoftStyle,
    closed: bool,
    ends: viboceros_geometry::LoftEndPoints,
    fit_tolerance: f64,
    angle: f64,
) -> Result<NurbsSurface, Error> {
    refitted(curves, style, closed, ends, fit_tolerance, Some(angle))
}

fn refitted(
    curves: &[NurbsCurve],
    style: viboceros_geometry::LoftStyle,
    closed: bool,
    ends: viboceros_geometry::LoftEndPoints,
    fit_tolerance: f64,
    angle: Option<f64>,
) -> Result<NurbsSurface, Error> {
    if closed && !ends.is_empty() {
        return Err(viboceros_geometry::GeometryError::InvalidControlNet {
            context: "closed Loft cannot have point ends",
        }
        .into());
    }
    let mut prepare = |curves: &[NurbsCurve]| {
        viboceros_geometry::prepare_refitted_loft_sections_with_operations(
            curves,
            fit_tolerance,
            angle,
            &mut clamp_curve,
            &mut |c, d| change_curve_degree(c, d, false),
            &mut insert_curve_knot,
        )
    };
    if ends.is_empty() {
        viboceros_geometry::try_loft_nurbs_curves_with_operations(
            curves,
            style,
            closed,
            &mut prepare,
            &mut solve_cubic_system,
        )
    } else {
        viboceros_geometry::try_loft_nurbs_curves_with_end_points_and_operations(
            curves,
            style,
            ends,
            &mut prepare,
            &mut solve_cubic_system,
        )
    }
}
