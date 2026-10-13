//! Exact edge blends around existing native parent preparation operations.
use super::*;
use viboceros_geometry::{
    SurfaceBlendOperations, SurfaceBlendOptions, SurfaceBlendResult, SurfaceExtensionEdge,
    SurfaceKnotDirection,
};

/// Clamp both active axes through the existing checked native knot edits.
fn clamp(source: &NurbsSurface) -> Result<NurbsSurface, Error> {
    if source.degree_u() > 17 || source.degree_v() > 17 {
        return Ok(source.try_clamped_to_active_domain()?);
    }
    let mut result = source.clone();
    for direction in [SurfaceKnotDirection::U, SurfaceKnotDirection::V] {
        result = super::knot_refinement::clamp_surface_direction(&result, direction)?;
    }
    Ok(result)
}

/// Preserve exact homogeneous Hermite constraints while delegating clamping,
/// boundary degree elevation and knot matching to the qualified native edits.
pub fn blend_surfaces(
    first: &NurbsSurface,
    first_edge: SurfaceExtensionEdge,
    second: &NurbsSurface,
    second_edge: SurfaceExtensionEdge,
    options: SurfaceBlendOptions,
    tolerance: Tolerance,
) -> Result<SurfaceBlendResult, Error> {
    viboceros_geometry::try_blend_nurbs_surfaces_with_operations(
        first,
        first_edge,
        second,
        second_edge,
        options,
        tolerance,
        &mut SurfaceBlendOperations {
            clamp: &mut clamp,
            elevate: &mut |source, degree| {
                if source.degree_u() > 17
                    || source.degree_v() > 17
                    || degree.into_iter().any(|p| p > 17)
                {
                    return Ok(source.try_change_degree(degree[0], degree[1], false)?);
                }
                change_surface_degree(source, degree[0], degree[1], false)
            },
            refine: &mut |source, parameter, target| {
                if source.degree_u() > 17 || source.degree_v() > 17 {
                    return Ok(source.try_insert_knot_u(parameter, target)?);
                }
                insert_surface_knot(source, SurfaceKnotDirection::U, parameter, target)
            },
        },
    )
}

/// Rectangular BlendSrf parent restrictions use the same native trim adapter.
pub fn trim_blend_parent(
    source: &NurbsSurface,
    u: std::ops::RangeInclusive<f64>,
    v: std::ops::RangeInclusive<f64>,
) -> Result<NurbsSurface, Error> {
    if source.degree_u() > 17 || source.degree_v() > 17 {
        return Ok(source.try_trimmed(u, v)?);
    }
    trim_surface(source, u, v)
}
