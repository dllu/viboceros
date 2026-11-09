//! Ordinary regular NURBS ribbons with original boundary gauges and clocks.
use super::*;
use crate::curve_offset::{fit_ribbon_offset, nurbs_offset_plane};
mod seed;

/// Construct an ordinary planar smooth-curve ribbon. Lines translate their
/// original basis; a control-polygon offset candidate retains its knots and
/// weights when interior fit checks accept it. Other regular spans use cubic
/// Hermite offsets with polynomial weights.
/// U is [0,abs(distance)], V retains the original chart. BothSides shares the
/// source boundary; projective source/offset gauges remain independent.
///
/// Interior fitting checks do not certify constant distance between stations.
/// Kinked sources, cusps, self-intersection trimming and command policy require
/// separate handling; this does not implement the complete Ribbon command.
pub fn try_smooth_ribbon(
    source: &NurbsCurve,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    try_smooth_ribbon_with_ruled(
        source,
        distance,
        both_sides,
        normal,
        tolerance,
        &mut NurbsSurface::try_ruled_from_curves,
    )
}

/// Delegate ruled degree/knot edits while sharing the offset fitting policy.
pub fn try_smooth_ribbon_with_ruled<E: From<GeometryError>>(
    source: &NurbsCurve,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    tolerance: Tolerance,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Brep, E> {
    try_smooth_ribbon_with_fit_tolerance(
        source, distance, both_sides, normal, tolerance, tolerance, ruled,
    )
}

/// Fit tolerance controls offset approximation only. Sewing and validation
/// retain the separate model tolerance, even when the requested fit is coarse.
#[allow(clippy::too_many_arguments)]
pub fn try_smooth_ribbon_with_fit_tolerance<E: From<GeometryError>>(
    source: &NurbsCurve,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    model_tolerance: Tolerance,
    fit_tolerance: Tolerance,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Brep, E> {
    build_with_offset(
        source,
        distance,
        both_sides,
        model_tolerance,
        &mut |d| Ok(offset(source, d, normal, model_tolerance, fit_tolerance)?),
        ruled,
    )
}

fn offset(
    source: &NurbsCurve,
    distance: Real,
    fallback: UnitVector3,
    model_tolerance: Tolerance,
    fit_tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let normal = nurbs_offset_plane(source, fallback, model_tolerance)?;
    source.tight_bounds(model_tolerance)?;
    if let Some(candidate) = seed::try_offset(source, distance, normal, fit_tolerance)? {
        return Ok(candidate);
    }
    fit_ribbon_offset(source, distance, normal, fit_tolerance)
}

#[cfg(test)]
mod tests;
