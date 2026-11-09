//! Shared surface matching with owned native basis edits and interpolation.
use super::*;
use viboceros_geometry::{
    SurfaceAverageOptions, SurfaceExtensionEdge, SurfaceKnotDirection, SurfaceMatchOperations,
    SurfaceMatchOptions,
};

pub fn match_surface_supports(
    source: &NurbsSurface,
    side: SurfaceExtensionEdge,
    target: &NurbsSurface,
    target_side: SurfaceExtensionEdge,
    options: SurfaceMatchOptions,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    with_operations(|operations| {
        viboceros_geometry::try_match_nurbs_surfaces_with_operations(
            source,
            side,
            target,
            target_side,
            options,
            tolerance,
            operations,
        )
    })
}

pub fn average_match_surface_supports(
    first: &NurbsSurface,
    first_side: SurfaceExtensionEdge,
    second: &NurbsSurface,
    second_side: SurfaceExtensionEdge,
    options: SurfaceAverageOptions,
    tolerance: Tolerance,
) -> Result<[NurbsSurface; 2], Error> {
    with_operations(|operations| {
        viboceros_geometry::try_average_match_nurbs_surfaces_with_operations(
            first,
            first_side,
            second,
            second_side,
            options,
            tolerance,
            operations,
        )
    })
}

fn with_operations<R>(
    run: impl FnOnce(&mut SurfaceMatchOperations<'_, Error>) -> Result<R, Error>,
) -> Result<R, Error> {
    run(&mut SurfaceMatchOperations {
        clamp: &mut |s| {
            if s.degree_u() > 17 || s.degree_v() > 17 {
                return Ok(s.try_clamped_to_active_domain()?);
            }
            let u = super::knot_refinement::clamp_surface_direction(s, SurfaceKnotDirection::U)?;
            super::knot_refinement::clamp_surface_direction(&u, SurfaceKnotDirection::V)
        },
        elevate: &mut |s, d| {
            if s.degree_u() > 17 || s.degree_v() > 17 || d.into_iter().any(|p| p > 17) {
                return Ok(s.try_change_degree(d[0], d[1], false)?);
            }
            change_surface_degree(s, d[0], d[1], false)
        },
        interpolate: &mut super::knot_refinement::solve_surface_match_interpolation,
        refine: &mut |s, t| {
            if s.degree_u() > 17 || s.degree_v() > 17 {
                return Ok(s.try_insert_knot_v(t, 1)?);
            }
            insert_surface_knot(s, SurfaceKnotDirection::V, t, 1)
        },
    })
}
