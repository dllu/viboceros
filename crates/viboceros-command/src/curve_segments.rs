//! Native NURBS interval and seam operations shared by commands and previews.
use super::*;

/// Prepare a directed curve piece with the same backend used by SubCrv.
/// Sources and document state are unchanged, making this suitable for previews.
pub fn directed(curve: &Curve3, start: Real, end: Real) -> Result<Curve3, CommandError> {
    curve.try_subcurve_with_nurbs(start, end, &mut trim)
}

pub(super) fn length_with_endpoint(
    curve: &Curve3,
    anchor: Real,
    length: Real,
    tolerance: Tolerance,
) -> Result<Option<(Curve3, Real)>, CommandError> {
    curve.try_subcurve_at_arc_length_with_operations(
        anchor,
        length,
        tolerance,
        &mut trim,
        &mut nurbs_seam,
    )
}

fn nurbs_seam(curve: &NurbsCurve, parameter: Real) -> Result<NurbsCurve, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::change_curve_seam(curve, parameter)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(curve.try_change_closed_seam(parameter)?)
    }
}

pub(super) fn seam(curve: &Curve3, parameter: Real) -> Result<Curve3, CommandError> {
    curve.try_change_closed_seam_with_nurbs(parameter, &mut nurbs_seam, &mut trim)
}

pub(super) fn trimmed(
    curve: &Curve3,
    interval: std::ops::RangeInclusive<Real>,
) -> Result<Curve3, CommandError> {
    curve.try_trimmed_with_nurbs(interval, &mut trim)
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
        let result = viboceros_smlib::trim_curve(curve, interval)
            .map_err(knot_refinement::native_geometry_error)?;
        Ok(if curve.has_rational_representation() {
            result.with_rational_representation()
        } else {
            result
        })
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(curve.try_trimmed(interval)?)
    }
}
