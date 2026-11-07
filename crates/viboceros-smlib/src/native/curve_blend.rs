//! Curve Blend through the exact native endpoint interpolator shared by BlendSrf.
use super::*;
use viboceros_geometry::{Curve3, CurveBlendOptions};

pub fn blend_curve(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    options: CurveBlendOptions,
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    viboceros_geometry::try_blend_curve_with_interpolation(
        first,
        first_pick,
        second,
        second_pick,
        options,
        tolerance,
        &mut super::knot_refinement::interpolate_endpoints,
    )
}

/// Interpolate already oriented contacts with the same native endpoint solver.
pub fn blend_curve_from_jets(
    jets: [viboceros_geometry::CurveBlendJet; 2],
    options: CurveBlendOptions,
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    viboceros_geometry::try_blend_curve_from_jets_with_interpolation(
        jets,
        options,
        tolerance,
        &mut super::knot_refinement::interpolate_endpoints,
    )
}

/// Higher-order curve jets share the native derivative-basis solve.
pub fn blend_curve_from_endpoint_jets(
    jets: [viboceros_geometry::CurveBlendJet; 2],
    options: viboceros_geometry::CurveBlendJetOptions,
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    viboceros_geometry::try_blend_curve_from_endpoint_jets_with_interpolation(
        jets,
        options,
        tolerance,
        &mut super::knot_refinement::interpolate_endpoints,
    )
}
