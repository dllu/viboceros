//! Recovered walks fill outlines on the first complete boundary's plane.
use super::*;
use viboceros_geometry::{CurveRef, planar_curve_network_boundaries};

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
        // Retain the selected adapter's admission checks on every profile.
        stage_profiles(vec![profile.clone()], tolerance, false)?;
        used.push(i);
    }
    let boundaries = planar_curve_network_boundaries(&prepared.paths, plane, tolerance)?;
    let mut faces = Vec::new();
    for boundary in boundaries {
        faces.extend(stage_profiles(vec![boundary], tolerance, false)?.0);
    }
    if faces.is_empty() {
        return Err(CommandError::NoPlanarSurfaceBoundaries);
    }
    Ok((faces, used))
}
