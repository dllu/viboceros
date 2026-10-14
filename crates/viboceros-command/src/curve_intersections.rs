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
