//! Offset ribbons with ordered ruled tensors and shared corner identities.
mod corner;
mod in_plane;
mod polyline;
mod smooth;
use crate::{
    Brep, CurveContinuity, CurveEditPointSolver, GeometryError, NurbsCurve, NurbsSurface, Real,
    Tolerance, UnitVector3,
};
pub use corner::RibbonCornerStyle;
pub use in_plane::{try_loose_ribbon_in_plane, try_loose_ribbon_in_plane_with_ruled};
pub use polyline::{try_polyline_ribbon, try_polyline_ribbon_with_ruled};
pub use smooth::{
    try_smooth_ribbon, try_smooth_ribbon_with_fit_tolerance, try_smooth_ribbon_with_ruled,
};

/// Build a loose polynomial offset ribbon. U follows source-to-offset rulings;
/// V retains the source's chart. BothSides builds and sews two independent sides
/// at the original curve. G1 breaks split faces; closed and singular natural
/// boundaries use the shared surface topology constructor. This does not trim
/// self-intersections or claim native non-loose offset/corner policies.
pub fn try_loose_ribbon(
    source: &NurbsCurve,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    tolerance: Tolerance,
    solver: Option<CurveEditPointSolver>,
) -> Result<Brep, GeometryError> {
    try_loose_ribbon_with_ruled(
        source,
        distance,
        both_sides,
        normal,
        tolerance,
        solver,
        &mut NurbsSurface::try_ruled_from_curves,
    )
}

/// Share ribbon sampling/topology while delegating ordered ruled construction.
pub fn try_loose_ribbon_with_ruled<E: From<GeometryError>>(
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
        &mut |d| Ok(source.try_loose_polynomial_offset(d, normal, tolerance, solver)?),
        ruled,
    )
}

fn build_with_offset<E: From<GeometryError>>(
    source: &NurbsCurve,
    distance: Real,
    both_sides: bool,
    tolerance: Tolerance,
    offset: &mut impl FnMut(Real) -> Result<NurbsCurve, E>,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Brep, E> {
    if !distance.is_finite() || distance == 0. {
        return Err(GeometryError::InvalidCurveOffsetDistance.into());
    }
    let mut sides = Vec::with_capacity(if both_sides { 2 } else { 1 });
    for d in [distance, -distance]
        .into_iter()
        .take(if both_sides { 2 } else { 1 })
    {
        let offset = offset(d)?;
        let surface =
            ruled(source, &offset)?.try_reparameterized(0. ..=distance.abs(), source.domain())?;
        sides.push(Brep::try_surface_partitioned_at_discontinuities(
            &surface,
            &[(CurveContinuity::G1, 0.1_f64.to_radians())],
            tolerance,
        )?);
    }
    if !both_sides {
        return Ok(sides.remove(0));
    }
    let mut joined = crate::brep::join_breps_with_curve_witnesses(
        &sides.iter().collect::<Vec<_>>(),
        tolerance.absolute(),
        tolerance,
        &[source],
    )?;
    if joined.len() != 1 {
        return Err(GeometryError::RibbonJoinFailed.into());
    }
    Ok(joined.remove(0).brep)
}
