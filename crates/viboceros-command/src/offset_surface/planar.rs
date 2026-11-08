//! Portable planar offsets and closed wall construction.
use super::*;

#[cfg(any(not(feature = "native-smlib"), test))]
pub(super) fn planar_normal(
    geometry: &Geometry,
    tolerance: Tolerance,
) -> Result<Vector3, CommandError> {
    let (surface, reversed) = match geometry {
        Geometry::NurbsSurface(surface) => (surface, false),
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            (face.surface(), face.is_reversed())
        }
        _ => return Err(CommandError::OffsetSurfaceRequiresPlanarFaces),
    };
    let plane = surface
        .plane(tolerance)?
        .ok_or(CommandError::OffsetSurfaceRequiresPlanarFaces)?;
    Ok(plane
        .normal()
        .as_vector()
        .scaled(if reversed { -1.0 } else { 1.0 })?)
}

#[cfg(not(feature = "native-smlib"))]
pub(super) fn solid_profile(
    geometry: &Geometry,
    tolerance: Tolerance,
) -> Result<NurbsCurve, CommandError> {
    let loops = match geometry {
        Geometry::NurbsSurface(surface) => surface.natural_boundary_curve_loops()?,
        Geometry::Brep(brep) => brep.face_boundary_curve_components(0)?,
        _ => return Err(CommandError::OffsetSurfaceRequiresPlanarFaces),
    };
    let [boundary] = loops.as_slice() else {
        return Err(CommandError::OffsetSurfaceSolidRequiresOneLoop);
    };
    border::assemble(boundary.clone(), tolerance)?
        .nurbs_curve_representation()?
        .ok_or(CommandError::OffsetSurfaceSolidRequiresOneLoop)
}

#[cfg(not(feature = "native-smlib"))]
pub(super) fn rectangular_solid(
    geometry: &Geometry,
    normal: Vector3,
    distance: Real,
    both_sides: bool,
    tolerance: Tolerance,
) -> Result<Option<Brep>, CommandError> {
    let (surface, u, v) = match geometry {
        Geometry::NurbsSurface(s) => (s, s.domain_u(), s.domain_v()),
        Geometry::Brep(b) if b.faces().len() == 1 => {
            let face = &b.faces()[0];
            let Some(bounds) = face.rectangular_trim_bounds(tolerance)? else {
                return Ok(None);
            };
            (
                face.surface(),
                bounds[0][0]..=bounds[0][1],
                bounds[1][0]..=bounds[1][1],
            )
        }
        _ => return Ok(None),
    };
    if surface.degree_u() != 1
        || surface.degree_v() != 1
        || surface.control_point_count_u() != 2
        || surface.control_point_count_v() != 2
    {
        return Ok(None);
    }
    let origin = surface.evaluate(*u.start(), *v.start())?;
    let east = surface.evaluate(*u.end(), *v.start())?;
    let north = surface.evaluate(*u.start(), *v.end())?;
    let opposite = surface.evaluate(*u.end(), *v.end())?;
    let frame = Frame3::try_from_points(origin, east, north, tolerance)?;
    let east_local = frame.coordinates_of(east)?;
    let north_local = frame.coordinates_of(north)?;
    let opposite_local = frame.coordinates_of(opposite)?;
    let width = east_local[0];
    let height = north_local[1];
    let residuals = [
        east_local[1],
        east_local[2],
        north_local[0],
        north_local[2],
        opposite_local[0] - width,
        opposite_local[1] - height,
        opposite_local[2],
    ];
    if width <= tolerance.absolute()
        || height <= tolerance.absolute()
        || residuals
            .iter()
            .any(|value| value.abs() > tolerance.absolute())
    {
        return Ok(None);
    }
    let signed_height = distance * normal.dot(frame.z_axis().as_vector())?;
    let z = if both_sides {
        [-signed_height.abs(), signed_height.abs()]
    } else {
        [signed_height.min(0.0), signed_height.max(0.0)]
    };
    Ok(Some(Brep::try_box(
        frame,
        [[0.0, width], [0.0, height], z],
        tolerance,
    )?))
}
