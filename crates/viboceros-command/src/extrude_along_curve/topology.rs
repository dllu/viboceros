//! Reuse geometric continuity, exact shared-edge grids, and planar hole capping.
use super::*;

pub(super) fn cuts(
    curve: &NurbsCurve,
    crease: bool,
    split: bool,
) -> Result<Vec<f64>, CommandError> {
    // The command's G2 option uses one degree. The global crease policy
    // independently adds G1 breaks above 0.1 degree; neither query subsumes
    // the other near their angular and curvature thresholds.
    let mut cuts = if split {
        curve
            .discontinuity_parameters(viboceros_geometry::CurveContinuity::G2, 1_f64.to_radians())?
    } else {
        vec![]
    };
    if crease {
        cuts.extend(curve.discontinuity_parameters(
            viboceros_geometry::CurveContinuity::G1,
            0.1_f64.to_radians(),
        )?);
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    Ok(cuts)
}

pub(super) fn finish(
    wall: NurbsSurface,
    profile: &NurbsCurve,
    path: &NurbsCurve,
    cuts: [&[f64]; 2],
    solid: bool,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let [profile_cuts, path_cuts] = cuts;
    if !solid && profile_cuts.is_empty() && path_cuts.is_empty() {
        return Ok(Geometry::NurbsSurface(wall));
    }
    let brep = Brep::try_surface_grid(&wall, profile_cuts, path_cuts, tolerance)?;
    if solid {
        if brep.is_solid() {
            return Err(GeometryError::InvalidCappedExtrusionPath.into());
        }
        if let Some(capped) = brep.try_cap_planar_holes(tolerance)? {
            return Ok(Geometry::Brep(capped));
        }
        if profile.is_closed()? && !path.is_closed()? && profile.is_planar(tolerance)? {
            return Err(GeometryError::CoplanarCappedExtrusion.into());
        }
    }
    Ok(if profile_cuts.is_empty() && path_cuts.is_empty() {
        Geometry::NurbsSurface(wall)
    } else {
        Geometry::Brep(brep)
    })
}
