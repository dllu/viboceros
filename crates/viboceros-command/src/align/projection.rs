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
    let objects = document.selected_objects().collect::<Vec<_>>();
    let count = objects.len();
    if options.mode == Some(AlignmentMode::ToFitPlane) {
        if count < 3 {
            return Err(CommandError::InsufficientPlaneAlignmentObjects { actual: count });
        }
        if count > viboceros_geometry::MAX_PLANE_FIT_POINTS {
            return Err(viboceros_geometry::GeometryError::PlaneFitResourceLimit {
                maximum: viboceros_geometry::MAX_PLANE_FIT_POINTS,
            }
            .into());
        }
    }
    let anchors = objects
        .iter()
        .map(|object| {
            // Each object gets a geometry-local arithmetic origin, independently
            // of the display CPlane and of other selected objects.
            let frame = orientation.with_origin(object.geometry().bounds().center()?);
            let bounds = crate::object_bounds::local_bounds(
                [object.geometry()],
                frame,
                document.tolerance(),
            )?;
            let mut local = bounds.center()?.to_array();
            local[2] = bounds.min().z();
            frame.point_at(local)
        })
        .collect::<Result<Vec<_>, viboceros_geometry::GeometryError>>()?;
    let projector = match options.mode {
        Some(AlignmentMode::ToFitPlane) => PointProjection3::onto_best_fit_plane(&anchors)?,
        Some(AlignmentMode::ToLine) => PointProjection3::onto_line(
            options.references[0].unwrap(),
            options.references[1].unwrap(),
        )?,
        Some(AlignmentMode::ToPlane) => {
            let [Some(a), Some(b), c] = options.references else {
                return Err(CommandError::Usage(USAGE));
            };
            if let Some(c) = c {
                PointProjection3::onto_three_point_plane([a, b, c])?
            } else {
                PointProjection3::onto_plane_parallel_to(a, b, orientation.z_axis().as_vector())?
            }
        }
        _ => return Err(CommandError::Usage(USAGE)),
    };
    let mut replacements = Vec::new();
    for (object, anchor) in objects.iter().zip(anchors) {
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
