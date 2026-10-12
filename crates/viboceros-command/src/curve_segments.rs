//! Exact NURBS segment extraction, shared by overlap and surface-cut outputs.
use super::*;

/// Prepare a directed curve piece with the same backend used by SubCrv.
/// Sources and document state are unchanged, making this suitable for previews.
pub fn directed(curve: &Curve3, start: Real, end: Real) -> Result<Curve3, CommandError> {
    curve.try_subcurve_with_nurbs(start, end, &mut trim)
}

pub(super) fn split(curve: &Curve3, parameters: &[Real]) -> Result<Vec<Curve3>, CommandError> {
    curve.try_split_at_parameters_with_nurbs(parameters, &mut trim)
}

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
