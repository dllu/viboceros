use crate::{CommandError, CurveCurveIntersectionEvent, NurbsCurve, Tolerance};

#[cfg(all(test, feature = "native-smlib"))]
mod tests;

pub(crate) fn events(
    first: &NurbsCurve,
    second: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Vec<CurveCurveIntersectionEvent>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        Ok(viboceros_smlib::curve_intersection_events(
            first, second, tolerance,
        )?)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(first.intersection_events_with_curve(second, tolerance)?)
    }
}

pub(crate) fn surface_events(
    curve: &NurbsCurve,
    surface: &crate::NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<crate::CurveSurfaceIntersectionEvent>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        Ok(viboceros_smlib::curve_surface_intersection_events(
            curve, surface, tolerance,
        )?)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::curve_surface_intersection_events(
            curve, surface, tolerance,
        )?)
    }
}

pub(crate) fn brep_events(
    curve: &NurbsCurve,
    brep: &crate::Brep,
    transform: Option<crate::AffineTransform3>,
    tolerance: Tolerance,
) -> Result<Vec<crate::CurveBrepIntersectionEvent>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        Ok(viboceros_smlib::curve_brep_intersection_events(
            curve, brep, transform, tolerance,
        )?)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(if let Some(transform) = transform {
            viboceros_geometry::transformed_curve_brep_intersection_events(
                curve, brep, transform, tolerance,
            )?
        } else {
            viboceros_geometry::curve_brep_intersection_events(curve, brep, tolerance)?
        })
    }
}

pub(crate) fn surface_pair_events(
    first: &crate::NurbsSurface,
    second: &crate::NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<crate::SurfaceSurfaceIntersectionEvent>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::modeling_surface_intersection_events(first, second, tolerance)
            .map_err(surface_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::surface_surface_intersection_events(
            first, second, tolerance,
        )?)
    }
}
pub(crate) fn surface_brep_events(
    surface: &crate::NurbsSurface,
    brep: &crate::Brep,
    tolerance: Tolerance,
) -> Result<Vec<crate::SurfaceBrepIntersectionEvent>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::surface_brep_intersection_events(surface, brep, tolerance)
            .map_err(surface_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::surface_brep_intersection_events(
            surface, brep, tolerance,
        )?)
    }
}
pub(crate) fn brep_pair_events(
    first: &crate::Brep,
    second: &crate::Brep,
    tolerance: Tolerance,
) -> Result<Vec<crate::BrepBrepIntersectionEvent>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::brep_intersection_events(first, second, tolerance).map_err(surface_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::brep_brep_intersection_events(
            first, second, tolerance,
        )?)
    }
}

#[cfg(feature = "native-smlib")]
fn surface_error(error: viboceros_smlib::Error) -> CommandError {
    match error {
        viboceros_smlib::Error::Geometry(error) => error.into(),
        error => error.into(),
    }
}
