use crate::{CommandError, CurveCurveIntersectionEvent, NurbsCurve, Tolerance};

#[cfg(test)]
mod tests;

pub(crate) fn events(
    first: &NurbsCurve,
    second: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Vec<CurveCurveIntersectionEvent>, CommandError> {
    Ok(viboceros_smlib::curve_intersection_events(
        first, second, tolerance,
    )?)
}

pub(crate) fn surface_events(
    curve: &NurbsCurve,
    surface: &crate::NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<crate::CurveSurfaceIntersectionEvent>, CommandError> {
    Ok(viboceros_smlib::curve_surface_intersection_events(
        curve, surface, tolerance,
    )?)
}

pub(crate) fn brep_events(
    curve: &NurbsCurve,
    brep: &crate::Brep,
    transform: Option<crate::AffineTransform3>,
    tolerance: Tolerance,
) -> Result<Vec<crate::CurveBrepIntersectionEvent>, CommandError> {
    Ok(viboceros_smlib::curve_brep_intersection_events(
        curve, brep, transform, tolerance,
    )?)
}

pub(crate) fn surface_pair_events(
    first: &crate::NurbsSurface,
    second: &crate::NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<crate::SurfaceSurfaceIntersectionEvent>, CommandError> {
    viboceros_smlib::modeling_surface_intersection_events(first, second, tolerance)
        .map_err(surface_error)
}
pub(crate) fn surface_brep_events(
    surface: &crate::NurbsSurface,
    brep: &crate::Brep,
    tolerance: Tolerance,
) -> Result<Vec<crate::SurfaceBrepIntersectionEvent>, CommandError> {
    viboceros_smlib::surface_brep_intersection_events(surface, brep, tolerance)
        .map_err(surface_error)
}
pub(crate) fn brep_pair_events(
    first: &crate::Brep,
    second: &crate::Brep,
    tolerance: Tolerance,
) -> Result<Vec<crate::BrepBrepIntersectionEvent>, CommandError> {
    viboceros_smlib::brep_intersection_events(first, second, tolerance).map_err(surface_error)
}

fn surface_error(error: viboceros_smlib::Error) -> CommandError {
    match error {
        viboceros_smlib::Error::Geometry(error) => error.into(),
        error => error.into(),
    }
}
