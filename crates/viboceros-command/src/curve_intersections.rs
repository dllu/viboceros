use crate::{CommandError, CurveCurveIntersectionEvent, NurbsCurve, Tolerance};

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
