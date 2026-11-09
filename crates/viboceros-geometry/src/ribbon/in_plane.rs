//! Fixed-normal loose loci retain the source's spatial heights.
use super::*;

pub fn try_loose_ribbon_in_plane(
    source: &NurbsCurve,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    tolerance: Tolerance,
    solver: Option<CurveEditPointSolver>,
) -> Result<Brep, GeometryError> {
    try_loose_ribbon_in_plane_with_ruled(
        source,
        distance,
        both_sides,
        normal,
        tolerance,
        solver,
        &mut NurbsSurface::try_ruled_from_curves,
    )
}

/// Ribbon Loose/InCPlane uses a fixed normal even for spatial source curves.
/// Greville positions keep their heights; interpolation remains a polynomial
/// approximation without a constant-distance certificate between stations.
pub fn try_loose_ribbon_in_plane_with_ruled<E: From<GeometryError>>(
    source: &NurbsCurve,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    tolerance: Tolerance,
    solver: Option<CurveEditPointSolver>,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Brep, E> {
    build_with_offset(
        source,
        distance,
        both_sides,
        tolerance,
        &mut |d| Ok(source.try_loose_polynomial_offset_in_plane(d, normal, tolerance, solver)?),
        ruled,
    )
}
