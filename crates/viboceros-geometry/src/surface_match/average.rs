//! Two independent averaging edits retain each support's own control basis.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceAverageOptions {
    pub matching: SurfaceMatchOptions,
    /// Independent traversal flag for the second surface's edit.
    pub reverse_target: bool,
}

/// Average open natural boundaries with position, tangency or curvature continuity.
/// Each half retains its source's control basis. The two traversal
/// flags are independent, following the public SDK, and can yield different
/// boundary curves. Averaged refinement is not supported yet.
pub fn try_average_match_nurbs_surfaces(
    first: &NurbsSurface,
    first_side: SurfaceExtensionEdge,
    second: &NurbsSurface,
    second_side: SurfaceExtensionEdge,
    options: SurfaceAverageOptions,
    tolerance: Tolerance,
) -> Result<[NurbsSurface; 2], GeometryError> {
    try_average_match_nurbs_surfaces_with_operations(
        first,
        first_side,
        second,
        second_side,
        options,
        tolerance,
        &mut SurfaceMatchOperations {
            clamp: &mut NurbsSurface::try_clamped_to_active_domain,
            elevate: &mut |s, d| s.try_change_degree(d[0], d[1], false),
            interpolate: &mut crate::surface_rebuild::portable_solve,
            refine: &mut |s, t| s.try_insert_knot_v(t, 1),
        },
    )
}

pub fn try_average_match_nurbs_surfaces_with_operations<E: From<GeometryError>>(
    first: &NurbsSurface,
    first_side: SurfaceExtensionEdge,
    second: &NurbsSurface,
    second_side: SurfaceExtensionEdge,
    options: SurfaceAverageOptions,
    tolerance: Tolerance,
    operations: &mut SurfaceMatchOperations<'_, E>,
) -> Result<[NurbsSurface; 2], E> {
    if options.matching.refine {
        return Err(error("averaging requires refinement off").into());
    }
    let first_output = half(
        first,
        first_side,
        second,
        second_side,
        options.matching,
        tolerance,
        operations,
    )?;
    let mut policy = options.matching;
    policy.reverse_match = options.reverse_target;
    let second_output = half(
        second,
        second_side,
        first,
        first_side,
        policy,
        tolerance,
        operations,
    )?;
    Ok([first_output, second_output])
}

fn half<E: From<GeometryError>>(
    source: &NurbsSurface,
    side: SurfaceExtensionEdge,
    target: &NurbsSurface,
    target_side: SurfaceExtensionEdge,
    options: SurfaceMatchOptions,
    tolerance: Tolerance,
    operations: &mut SurfaceMatchOperations<'_, E>,
) -> Result<NurbsSurface, E> {
    let prepared = prepare::operands(source, side, target, target_side, options, operations)?;
    let mean = fit::average(
        &prepared.source,
        &prepared.target,
        prepared.reverse,
        options,
        tolerance,
        operations.interpolate,
    )?;
    Ok(prepared.chart.restore(&mean)?)
}
