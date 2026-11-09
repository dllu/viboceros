//! Two independent midpoint edits retain each support's own rational basis.
use super::*;
use crate::WeightedPoint3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceAverageOptions {
    pub matching: SurfaceMatchOptions,
    /// Independent traversal flag for the second surface's edit.
    pub reverse_target: bool,
}

/// Average open natural boundaries with position or tangency continuity.
/// Each half moves its own source toward a one-sided match. The two traversal
/// flags are independent, following the public SDK, and can yield different
/// boundary curves. Averaged curvature and refinement are not supported yet.
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
    if options.matching.continuity == SurfaceBlendContinuity::Curvature || options.matching.refine {
        return Err(error("averaging requires Position or Tangency with refinement off").into());
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
    let source = prepared.source;
    let matched = fit::fit(
        &source,
        &prepared.target,
        prepared.reverse,
        options,
        tolerance,
        operations.interpolate,
    )?;
    // G0/G1 matching retains the source's weight functions. Midpoints in the
    // common control basis therefore halve the physical edit as well.
    let controls = source
        .control_points()
        .iter()
        .zip(matched.control_points())
        .map(|(a, b)| {
            WeightedPoint3::try_new(
                a.point().midpoint(b.point())?,
                a.weight().midpoint(b.weight()),
            )
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mean = NurbsSurface::try_new_rational(
        source.degree_u(),
        source.degree_v(),
        source.control_point_count_u(),
        source.control_point_count_v(),
        controls,
        source.knots_u().to_vec(),
        source.knots_v().to_vec(),
    )?;
    Ok(prepared.chart.restore(&mean)?)
}
