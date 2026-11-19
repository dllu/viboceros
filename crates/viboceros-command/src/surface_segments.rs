//! Shared native surface extraction used by splitting, extension and pipe caps.
use super::*;
use std::ops::RangeInclusive;

pub(super) fn isocurve(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: Real,
) -> Result<NurbsCurve, CommandError> {
    viboceros_smlib::surface_isocurve(source, direction, parameter)
        .map_err(knot_refinement::native_geometry_error)
}

pub(super) fn trimmed_direction(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    range: RangeInclusive<Real>,
) -> Result<NurbsSurface, CommandError> {
    viboceros_smlib::trim_surface_direction(source, direction, range)
        .map_err(knot_refinement::native_geometry_error)
}

pub(super) fn trimmed(
    source: &NurbsSurface,
    u: RangeInclusive<Real>,
    v: RangeInclusive<Real>,
) -> Result<NurbsSurface, CommandError> {
    source.try_trimmed_with_extractor(u, v, &mut trimmed_direction)
}

pub(super) fn split(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: Real,
) -> Result<(NurbsSurface, NurbsSurface), CommandError> {
    viboceros_smlib::split_surface(source, direction, parameter)
        .map_err(knot_refinement::native_geometry_error)
}
