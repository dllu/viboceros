//! Delegate NURBS editing math while keeping command input and history policies.
use super::*;

pub(super) fn surface_seam(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    u: Real,
    v: Real,
) -> Result<NurbsSurface, CommandError> {
    viboceros_smlib::change_surface_seam(source, direction, u, v).map_err(native_geometry_error)
}

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
    let result = viboceros_smlib::remove_curve_multiple_knots(source, fully, angle)
        .map_err(native_geometry_error)?;
    Ok(result)
}

pub(super) fn remove_multiple_surface(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    fully: bool,
    angle: Real,
) -> Result<(NurbsSurface, usize), CommandError> {
    let result = viboceros_smlib::remove_surface_multiple_knots(source, direction, fully, angle)
        .map_err(native_geometry_error)?;
    Ok(result)
}

pub(super) fn remove_curve(
    source: &NurbsCurve,
    parameter: Real,
) -> Result<NurbsCurve, CommandError> {
    let result =
        viboceros_smlib::remove_curve_knot(source, parameter).map_err(native_geometry_error)?;
    Ok(result)
}

pub(super) fn remove_surface(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: Real,
) -> Result<NurbsSurface, CommandError> {
    let result = viboceros_smlib::remove_surface_knot(source, direction, parameter)
        .map_err(native_geometry_error)?;
    Ok(result)
}

pub(super) fn curve_degree(
    source: &NurbsCurve,
    degree: usize,
    deformable: bool,
) -> Result<NurbsCurve, CommandError> {
    let result = viboceros_smlib::change_curve_degree(source, degree, deformable)?;
    Ok(result)
}

pub(super) fn surface_degree(
    source: &NurbsSurface,
    u: usize,
    v: usize,
    deformable: bool,
) -> Result<NurbsSurface, CommandError> {
    let result = viboceros_smlib::change_surface_degree(source, u, v, deformable)?;
    Ok(result)
}

pub(super) fn non_periodic_curve(source: &NurbsCurve) -> Result<NurbsCurve, CommandError> {
    let result = viboceros_smlib::make_curve_non_periodic(source)?;
    Ok(result)
}

pub(super) fn non_periodic_surface(source: &NurbsSurface) -> Result<NurbsSurface, CommandError> {
    let result = viboceros_smlib::make_surface_non_periodic(source, SurfaceKnotDirection::Both)?;
    Ok(result)
}

pub(super) fn curve(
    source: &NurbsCurve,
    parameter: Real,
    target: usize,
) -> Result<NurbsCurve, CommandError> {
    let refined = viboceros_smlib::insert_curve_knot(source, parameter, target)?;
    Ok(refined)
}

pub(super) fn surface(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: Real,
    target: usize,
) -> Result<NurbsSurface, CommandError> {
    let refined = viboceros_smlib::insert_surface_knot(source, direction, parameter, target)?;
    Ok(refined)
}

pub(super) fn periodic_curve(
    source: &NurbsCurve,
    smooth: bool,
) -> Result<NurbsCurve, CommandError> {
    let result = viboceros_smlib::make_curve_periodic(source, smooth)?;
    Ok(result)
}

pub(super) fn periodic_surface(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    smooth: bool,
) -> Result<NurbsSurface, CommandError> {
    let result = viboceros_smlib::make_surface_periodic(source, direction, smooth)?;
    Ok(result)
}

pub(super) fn remove_curve_interactive(
    source: &NurbsCurve,
    parameter: Real,
) -> Result<NurbsCurve, CommandError> {
    let result = viboceros_smlib::remove_curve_knot_at_value(source, parameter)
        .map_err(native_geometry_error)?;
    // Constant homogeneous weights remain exactly constant under basis fitting.
    if !source.has_rational_representation() {
        Ok(NurbsCurve::try_new(
            result.degree(),
            result.control_points().iter().map(|p| p.point()).collect(),
            result.knots().to_vec(),
        )?)
    } else {
        Ok(result)
    }
}

pub(super) fn remove_surface_interactive(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: Real,
) -> Result<NurbsSurface, CommandError> {
    let periodic = match direction {
        SurfaceKnotDirection::U => source.is_periodic_u(),
        _ => source.is_periodic_v(),
    };
    if !periodic {
        return remove_surface(source, direction, parameter);
    }
    {
        viboceros_smlib::remove_periodic_surface_knot(source, direction, parameter)
            .map_err(native_geometry_error)
    }
}
