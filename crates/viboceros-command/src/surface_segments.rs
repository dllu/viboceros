//! Shared native surface extraction used by splitting, extension and pipe caps.
use super::*;
use std::ops::RangeInclusive;

pub(super) fn trimmed_direction(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    range: RangeInclusive<Real>,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::trim_surface_direction(source, direction, range)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(
            source.try_trimmed_direction_with_nurbs(direction, range, &mut |curve, range| {
                curve.try_trimmed(range)
            })?,
        )
    }
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
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::split_surface(source, direction, parameter)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        match direction {
            SurfaceKnotDirection::U => Ok(source.try_split_u(parameter)?),
            SurfaceKnotDirection::V => Ok(source.try_split_v(parameter)?),
            SurfaceKnotDirection::Both => Err(GeometryError::InvalidControlNet {
                context: "directional extraction requires one surface axis",
            }
            .into()),
        }
    }
}
