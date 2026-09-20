//! Projection modes translate each selected object's own bottom-center anchor.
use super::*;
use viboceros_geometry::PointProjection3;

pub(super) fn run(
    document: &mut Document,
    options: AlignmentOptions,
    context: CommandContext,
) -> Result<String, CommandError> {
    if !options.ready() {
        return Err(CommandError::Usage(USAGE));
    }
    let orientation = if options.world {
        CommandContext::default()
    } else {
        context
    }
    .construction_plane;
    let [Some(a), Some(b), c] = options.references else {
        return Err(CommandError::Usage(USAGE));
    };
    let projector = match options.mode {
        Some(AlignmentMode::ToLine) => PointProjection3::onto_line(a, b)?,
        Some(AlignmentMode::ToPlane) => {
            if let Some(c) = c {
                PointProjection3::onto_three_point_plane([a, b, c])?
            } else {
                PointProjection3::onto_plane_parallel_to(a, b, orientation.z_axis().as_vector())?
            }
        }
        _ => return Err(CommandError::Usage(USAGE)),
    };
    let mut replacements = Vec::new();
    let mut count = 0;
    for object in document.selected_objects() {
        count += 1;
        // Use a separate geometry-local origin for each object. A distant
        // display origin or another selected object cannot erase its extents.
        let frame = orientation.with_origin(object.geometry().bounds().center()?);
        let bounds =
            crate::object_bounds::local_bounds([object.geometry()], frame, document.tolerance())?;
        let mut local = bounds.center()?.to_array();
        local[2] = bounds.min().z();
        let anchor = frame.point_at(local)?;
        let projected = projector.project(anchor)?;
        if projected == anchor {
            continue;
        }
        let transform = AffineTransform3::from_translation(anchor.vector_to(projected)?);
        replacements.push((
            object.id(),
            object
                .geometry()
                .transformed(transform, document.tolerance())?,
        ));
    }
    if count == 0 {
        return Err(CommandError::NoObjectsSelected);
    }
    let changed = document.replace_object_geometries(replacements)?;
    Ok(format!(
        "Aligned {count} object(s) {}; moved {changed} object(s)",
        options.mode.unwrap().label()
    ))
}
