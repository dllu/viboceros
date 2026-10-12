//! Exact NURBS segment extraction, shared by overlap and surface-cut outputs.
use super::*;

pub(super) fn trim(
    curve: &NurbsCurve,
    interval: std::ops::RangeInclusive<Real>,
) -> Result<NurbsCurve, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::trim_curve(curve, interval).map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(curve.try_trimmed(interval)?)
    }
}
