//! Delegate NURBS editing math while keeping command input and history policies.
use super::*;

pub(super) fn surface_seam(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    u: Real,
    v: Real,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::change_surface_seam(source, direction, u, v).map_err(native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(source.try_change_closed_seam(direction, u, v)?)
    }
}

#[cfg(feature = "native-smlib")]
pub(super) fn native_geometry_error(error: viboceros_smlib::Error) -> CommandError {
    match error {
        viboceros_smlib::Error::Geometry(error) => error.into(),
        error => error.into(),
    }
}

pub(super) fn remove_multiple_curve(
    source: &NurbsCurve,
    fully: bool,
    angle: Real,
) -> Result<(NurbsCurve, usize), CommandError> {
    #[cfg(feature = "native-smlib")]
    let result = viboceros_smlib::remove_curve_multiple_knots(source, fully, angle)
        .map_err(native_geometry_error)?;
    #[cfg(not(feature = "native-smlib"))]
    let result = source.try_remove_multiple_knots(fully, angle)?;
    Ok(result)
}

pub(super) fn remove_multiple_surface(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    fully: bool,
    angle: Real,
) -> Result<(NurbsSurface, usize), CommandError> {
    #[cfg(feature = "native-smlib")]
    let result = viboceros_smlib::remove_surface_multiple_knots(source, direction, fully, angle)
        .map_err(native_geometry_error)?;
    #[cfg(not(feature = "native-smlib"))]
    let result = source.try_remove_multiple_knots(direction, fully, angle)?;
    Ok(result)
}

pub(super) fn remove_curve(
    source: &NurbsCurve,
    parameter: Real,
) -> Result<NurbsCurve, CommandError> {
    #[cfg(feature = "native-smlib")]
    let result =
        viboceros_smlib::remove_curve_knot(source, parameter).map_err(native_geometry_error)?;
    #[cfg(not(feature = "native-smlib"))]
    let result = source.try_remove_knot_near(parameter)?;
    Ok(result)
}

pub(super) fn remove_surface(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: Real,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    let result = viboceros_smlib::remove_surface_knot(source, direction, parameter)
        .map_err(native_geometry_error)?;
    #[cfg(not(feature = "native-smlib"))]
    let result = match direction {
        SurfaceKnotDirection::U => source.try_remove_knot_u_near(parameter)?,
        SurfaceKnotDirection::V => source.try_remove_knot_v_near(parameter)?,
        SurfaceKnotDirection::Both => unreachable!("command removal requires one direction"),
    };
    Ok(result)
}

pub(super) fn curve_degree(
    source: &NurbsCurve,
    degree: usize,
    deformable: bool,
) -> Result<NurbsCurve, CommandError> {
    #[cfg(feature = "native-smlib")]
    let result = viboceros_smlib::change_curve_degree(source, degree, deformable)?;
    #[cfg(not(feature = "native-smlib"))]
    let result = source.try_change_degree(degree, deformable)?;
    Ok(result)
}

pub(super) fn surface_degree(
    source: &NurbsSurface,
    u: usize,
    v: usize,
    deformable: bool,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    let result = viboceros_smlib::change_surface_degree(source, u, v, deformable)?;
    #[cfg(not(feature = "native-smlib"))]
    let result = source.try_change_degree(u, v, deformable)?;
    Ok(result)
}

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
