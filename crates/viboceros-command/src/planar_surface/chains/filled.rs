//! Recovered walks fill outlines on the first complete boundary's plane.
use super::*;
use viboceros_geometry::{BrepBooleanOperation, CurveRef, boolean_planar_breps};

pub(super) fn stage(
    prepared: &Prepared,
    tolerance: Tolerance,
) -> Result<(Vec<Brep>, Vec<usize>), CommandError> {
    let first = prepared.recovered.iter().position(|r| !*r).unwrap_or(0);
    let Some(reference) = prepared.profiles.get(first) else {
        return Err(CommandError::NoPlanarSurfaceBoundaries);
    };
    let plane = CurveRef::NurbsCurve(reference)
        .supporting_plane_frame(tolerance)?
        .ok_or(GeometryError::InvalidPlanarFaceBoundary)?;
    let mut faces = Vec::new();
    let mut used = Vec::new();
    for (i, profile) in prepared.profiles.iter().enumerate() {
        let coplanar = profile
            .control_points()
            .iter()
            .try_fold(true, |inside, p| {
                Ok::<_, GeometryError>(
                    inside && plane.coordinate_of(2, p.point())?.abs() <= tolerance.absolute(),
                )
            })?;
        if !coplanar {
            continue;
        }
        faces.extend(stage_profiles(vec![profile.clone()], tolerance, false)?.0);
        used.push(i);
    }
    if faces.len() == 1 {
        return Ok((faces, used));
    }
    Ok((
        boolean_planar_breps(
            &faces.iter().collect::<Vec<_>>(),
            BrepBooleanOperation::Union,
            tolerance,
        )?,
        used,
    ))
}
