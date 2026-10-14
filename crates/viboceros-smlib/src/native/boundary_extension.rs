//! Native exact producers within the shared boundary-hit extension policy.
use super::*;
use viboceros_geometry::{
    CurveExtensionBoundary, CurveExtensionOperations, CurveExtensionSide, CurveExtensionStyle,
};

pub fn extend_curve_to_boundaries(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    style: CurveExtensionStyle,
    boundaries: &[CurveExtensionBoundary],
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    if source.degree() > 17 {
        return Ok(source.try_merged_to_boundaries(side, style, boundaries, tolerance)?);
    }
    with_native_operations(|operations| {
        source.try_merged_to_boundaries_with_operations(
            side, style, boundaries, tolerance, operations,
        )
    })
}

pub fn join_curve_to_boundaries(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    style: CurveExtensionStyle,
    boundaries: &[CurveExtensionBoundary],
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    if source.degree() > 17 {
        return Ok(source.try_joined_to_boundaries(side, style, boundaries, tolerance)?);
    }
    with_native_operations(|operations| {
        source.try_joined_to_boundaries_with_operations(
            side, style, boundaries, tolerance, operations,
        )
    })
}

pub fn separate_curve_extensions_to_boundaries(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    style: CurveExtensionStyle,
    boundaries: &[CurveExtensionBoundary],
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, Error> {
    if source.degree() > 17 {
        return Ok(
            source.try_separate_extensions_to_boundaries(side, style, boundaries, tolerance)?
        );
    }
    with_native_operations(|operations| {
        source.try_separate_extensions_to_boundaries_with_operations(
            side, style, boundaries, tolerance, operations,
        )
    })
}

pub(super) fn with_native_operations<T>(
    run: impl FnOnce(&mut CurveExtensionOperations<'_, Error>) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut extend = extend_curve;
    let mut trim = |source: &NurbsCurve, interval| {
        if source.degree() > 17 {
            Ok(source.try_trimmed(interval)?)
        } else {
            trim_curve(source, interval)
        }
    };
    let mut clamp_endpoint = super::knot_refinement::clamp_curve_endpoint;
    run(&mut CurveExtensionOperations {
        extend: &mut extend,
        trim: &mut trim,
        clamp_endpoint: &mut clamp_endpoint,
    })
}
