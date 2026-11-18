//! Align world stations to a curve, line, or plane.
use super::*;
use viboceros_document::Geometry;
use viboceros_geometry::{PointProjection3, Tolerance};

pub(super) fn destinations(
    anchors: &[Point3],
    options: AlignmentOptions,
    orientation: Frame3,
    target: Option<&Geometry>,
    tolerance: Tolerance,
) -> Result<Vec<Point3>, CommandError> {
    let curve = target.and_then(Geometry::curve_ref);
    let projector = match options.mode {
        Some(AlignmentMode::ToCurve) => None,
        Some(AlignmentMode::ToFitPlane) => Some(PointProjection3::onto_best_fit_plane(anchors)?),
        Some(AlignmentMode::ToLine) => Some(PointProjection3::onto_line(
            options.references[0].unwrap(),
            options.references[1].unwrap(),
        )?),
        Some(AlignmentMode::ToPlane) => {
            let [Some(a), Some(b), c] = options.references else {
                return Err(CommandError::Usage(USAGE));
            };
            Some(if let Some(c) = c {
                PointProjection3::onto_three_point_plane([a, b, c])?
            } else {
                PointProjection3::onto_plane_parallel_to(a, b, orientation.z_axis().as_vector())?
            })
        }
        _ => return Err(CommandError::Usage(USAGE)),
    };
    anchors
        .iter()
        .map(|&point| {
            Ok(if let Some(curve) = curve {
                curve.evaluate(curve.closest_parameter(point, tolerance)?)?
            } else {
                projector
                    .as_ref()
                    .ok_or(CommandError::Usage(USAGE))?
                    .project(point)?
            })
        })
        .collect()
}
