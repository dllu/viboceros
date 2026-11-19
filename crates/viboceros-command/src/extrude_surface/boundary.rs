//! Stage straight surface sweeps that stop at a picked boundary.
use super::*;
use crate::surface_boundary_cut::{axial_extent, axial_point};
use viboceros_smlib::Shape;

mod section;

pub(super) fn apply(
    doc: &mut Document,
    faces: &SurfaceFaces,
    reference: BoundaryRef,
    options: SurfaceExtrusionOptions,
    cplane: Frame3,
) -> Result<usize, CommandError> {
    let boundary = reference.capture(doc)?;
    let tolerance = doc.tolerance();
    let selection = doc
        .selected_object_ids()
        .chain([reference.object])
        .collect::<Vec<_>>();
    let mut bounds = boundary.padded_trim_bounds()?;
    let mut sources = Vec::new();
    for (id, face) in surface_extrusion_targets(doc, faces)? {
        let brep = super::apply::capture_source_face(doc, id, face)?;
        let source_bounds = brep.padded_trim_bounds()?;
        bounds = bounds.union(source_bounds)?;
        sources.push(((id, face), brep));
    }
    let length = bounds.min().distance_to(bounds.max())?;
    let mut untrimmed = Vec::new();
    let mut finished = Vec::new();
    let mut used = Vec::new();
    let zero = Point3::try_new(0., 0., 0.)?;
    for (target, source) in sources {
        let (origin, source) = local_sweep_source(&source, tolerance)?;
        let cutter = boundary.transformed(
            AffineTransform3::from_translation(origin.vector_to(zero)?),
            tolerance,
        )?;
        let source_shape = Shape::from_brep(&source, tolerance)?;
        let direction =
            face_extrusion_direction(&source_shape, options.direction, cplane, tolerance)?;
        let restore = AffineTransform3::from_translation(zero.vector_to(origin)?);
        match extrude(
            &source,
            &cutter,
            direction,
            length,
            options.split_at_tangents,
            tolerance,
        )? {
            BoundarySweep::Finished(brep) => {
                finished.push(Geometry::Brep(brep.transformed(restore, tolerance)?));
                used.push(target);
            }
            BoundarySweep::Untrimmed(brep) => {
                untrimmed.push(Geometry::Brep(brep.transformed(restore, tolerance)?));
            }
            BoundarySweep::Miss => {}
        }
    }
    let count = finished.len();
    untrimmed.extend(finished);
    let outputs = super::apply::apply_staged_products(
        doc,
        untrimmed,
        used.into_iter().filter(|_| options.delete_input),
    )?;
    let selection = selection
        .into_iter()
        .filter(|&id| doc.object(id).is_some())
        .collect::<Vec<_>>();
    if !outputs.is_empty() {
        // On replay the cutter pick is released. Completed cuts release the
        // retained source picks as well. Removed inputs restore their saved selection.
        doc.release_command_selection_on_history_replay(
            selection
                .iter()
                .copied()
                .filter(|&id| count > 0 || id == reference.object),
        )?;
    }
    doc.select_command_results(selection)?;
    if count == 0 {
        return Err(CommandError::SurfaceExtrusionBoundaryRejected);
    }
    Ok(count)
}

#[derive(Debug)]
enum BoundarySweep {
    Finished(Brep),
    Untrimmed(Brep),
    Miss,
}

fn extrude(
    source: &Brep,
    boundary: &Brep,
    axis: Vector3,
    length: f64,
    split_at_tangents: bool,
    tolerance: Tolerance,
) -> Result<BoundarySweep, CommandError> {
    let axis = oriented_plane_normal(axis)?;
    if let Some(coincident) =
        coincident_sweep(source, boundary, axis, length, split_at_tangents, tolerance)?
    {
        return Ok(coincident);
    }
    let source_bounds = source.padded_trim_bounds()?;
    let cutter_bounds = boundary.padded_trim_bounds()?;
    let source_extent = axial_extent(source_bounds, axis)?;
    let target_extent = axial_extent(cutter_bounds, axis)?;
    let source_center = source_extent[0].midpoint(source_extent[1]);
    let target_center = target_extent[0].midpoint(target_extent[1]);
    let reversed = target_center < source_center;
    let direction = if reversed { axis.scaled(-1.)? } else { axis };
    let split_source = split_wall_edges(source, split_at_tangents, tolerance)?;
    let native = Shape::from_brep(&split_source, tolerance)?;
    let cut = |direction| {
        section::cut(
            source,
            &native,
            source_bounds,
            cutter_bounds,
            boundary,
            direction,
            length,
            tolerance,
        )
    };
    let result = cut(direction);
    match result {
        Ok(brep) => return Ok(BoundarySweep::Finished(brep)),
        Err(CommandError::SurfaceExtrusionBoundaryRejected) => {}
        Err(error) => return Err(error),
    }
    if source_extent[0] < target_extent[1] && target_extent[0] < source_extent[1] {
        match cut(direction.scaled(-1.)?) {
            Ok(brep) => return Ok(BoundarySweep::Finished(brep)),
            Err(CommandError::SurfaceExtrusionBoundaryRejected) => {}
            Err(error) => return Err(error),
        }
    }
    let source = split_wall_edges(source, false, tolerance)?;
    let native = Shape::from_brep(&source, tolerance)?;
    let displacement = direction.scaled(length)?;
    if reversed {
        // The reverse sweep is retained only when the cutter also crosses
        // the line swept from the source bounds center.
        let center = source_bounds.center()?;
        let ray = NurbsCurve::try_new(
            1,
            vec![center, center.translated(displacement)?],
            vec![0., 0., 1., 1.],
        )?;
        if viboceros_geometry::curve_brep_intersection_events(&ray, boundary, tolerance)?.is_empty()
        {
            return Ok(BoundarySweep::Miss);
        }
    }
    let walls = native
        .extruded_faces(displacement, false, tolerance)?
        .to_brep(tolerance)?;
    if viboceros_smlib::brep_intersection_events(&walls, boundary, tolerance)?.is_empty() {
        return Ok(BoundarySweep::Miss);
    }
    let solid = native
        .extruded_faces(displacement, true, tolerance)?
        .to_brep(tolerance)?;
    Ok(BoundarySweep::Untrimmed(solid))
}

fn coincident_sweep(
    source: &Brep,
    boundary: &Brep,
    axis: Vector3,
    length: f64,
    split_at_tangents: bool,
    tolerance: Tolerance,
) -> Result<Option<BoundarySweep>, CommandError> {
    let (Some(base), Some(cutter)) = (
        source.faces()[0].surface().plane(tolerance)?,
        boundary.faces()[0].surface().plane(tolerance)?,
    ) else {
        return Ok(None);
    };
    let normal = base.normal().as_vector();
    if normal.cross(cutter.normal().as_vector())?.length()? > tolerance.angular()
        || base.signed_distance_to(cutter.origin())?.abs() > tolerance.absolute()
    {
        return Ok(None);
    }
    for edge in source.edges() {
        if !viboceros_geometry::curve_brep_intersection_events(edge.curve(), boundary, tolerance)?
            .is_empty()
        {
            // The existing base already closes the section in this plane.
            let normal = oriented_plane_normal(normal)?;
            let distance = if normal.dot(axis)? > 0. {
                -length
            } else {
                length
            };
            let split = split_wall_edges(source, split_at_tangents, tolerance)?;
            let solid = Shape::from_brep(&split, tolerance)?
                .extruded_faces(axis.scaled(distance)?, true, tolerance)?
                .to_brep(tolerance)?;
            return Ok(Some(BoundarySweep::Finished(solid)));
        }
    }
    Ok(Some(BoundarySweep::Miss))
}

#[cfg(test)]
mod tests;
