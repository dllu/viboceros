//! Delegate insertion math while keeping command input and history policies.
use super::*;

pub(super) fn non_periodic_curve(source: &NurbsCurve) -> Result<NurbsCurve, CommandError> {
    #[cfg(feature = "native-smlib")]
    let result = viboceros_smlib::make_curve_non_periodic(source)?;
    #[cfg(not(feature = "native-smlib"))]
    let result = source.try_make_non_periodic()?;
    Ok(result)
}

pub(super) fn non_periodic_surface(source: &NurbsSurface) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    let result = viboceros_smlib::make_surface_non_periodic(source, SurfaceKnotDirection::Both)?;
    #[cfg(not(feature = "native-smlib"))]
    let result = source.try_make_non_periodic(SurfaceKnotDirection::Both)?;
    Ok(result)
}

pub(super) fn curve(
    source: &NurbsCurve,
    parameter: Real,
    target: usize,
) -> Result<NurbsCurve, CommandError> {
    #[cfg(feature = "native-smlib")]
    let refined = viboceros_smlib::insert_curve_knot(source, parameter, target)?;
    #[cfg(not(feature = "native-smlib"))]
    let refined = source.try_insert_knot(parameter, target)?;
    Ok(refined)
}

pub(super) fn surface(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: Real,
    target: usize,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    let refined = viboceros_smlib::insert_surface_knot(source, direction, parameter, target)?;
    #[cfg(not(feature = "native-smlib"))]
    let refined = match direction {
        SurfaceKnotDirection::U => source.try_insert_knot_u(parameter, target)?,
        SurfaceKnotDirection::V => source.try_insert_knot_v(parameter, target)?,
        SurfaceKnotDirection::Both => unreachable!("command insertion stages each direction"),
    };
    Ok(refined)
}
