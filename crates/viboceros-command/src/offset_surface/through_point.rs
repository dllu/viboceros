//! The closest selected underlying surface determines the shared signed distance.
use super::*;

pub fn through_distance(
    document: &Document,
    point: Point3,
    flip_all: bool,
    flips: &BTreeSet<ObjectId>,
) -> Result<Real, CommandError> {
    if document.selected_object_count() == 0 {
        return Err(CommandError::NoObjectsSelected);
    }
    let mut best: Option<(Real, Real)> = None;
    for source in document.selected_objects() {
        let surfaces = match source.geometry() {
            Geometry::NurbsSurface(s) => vec![(s, false)],
            Geometry::Brep(b) => b
                .faces()
                .iter()
                .map(|f| (f.surface(), f.is_reversed()))
                .collect(),
            _ => return Err(CommandError::OffsetSurfaceRequiresPlanarFaces),
        };
        for (surface, reversed) in surfaces {
            // Rhino uses the underlying surface even when the face is trimmed.
            let (u, v) = surface.closest_parameters(point, document.tolerance())?;
            let closest = surface.evaluate(u, v)?;
            let delta = closest.vector_to(point)?;
            let distance = closest.distance_to(point)?;
            let normal = surface.normal_at(u, v)?.as_vector();
            let reverse = reversed ^ flip_all ^ flips.contains(&source.id());
            let signed = if (delta.dot(normal)? < 0.) ^ reverse {
                -distance
            } else {
                distance
            };
            if best.is_none_or(|(old, _)| distance < old) {
                best = Some((distance, signed));
            }
        }
    }
    best.map(|(_, d)| d)
        .ok_or(CommandError::OffsetSurfaceRequiresPlanarFaces)
}
