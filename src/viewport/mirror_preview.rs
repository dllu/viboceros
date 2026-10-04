//! Mirror previews apply reflections to cached display samples, never the document.
use super::object_preview::TransformedObjects;
use super::*;
use viboceros_command::mirror::MirrorPointPlane;
use viboceros_geometry::AffineTransform3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MirrorPreview<'a> {
    pub plane: MirrorPointPlane,
    pub sources: &'a [ObjectId],
    pub grips: &'a [viboceros_document::ControlPointId],
    pub copy: bool,
    pub last_transform: Option<AffineTransform3>,
}

impl<'a> MirrorPreview<'a> {
    pub(super) fn resolve(
        self,
        cursor: Option<Point3>,
        frame: Frame3,
        tolerance: Tolerance,
    ) -> (
        Option<TransformedObjects<'a>>,
        Option<Option<AffineTransform3>>,
    ) {
        // Native Mirror retains its last valid shape at coincident or
        // collinear final points, without accepting an invalid plane.
        let update = cursor.map(|point| {
            self.plane
                .reflection_at(frame, point, tolerance)
                .ok()
                .or(self.last_transform)
        });
        let transform = update.unwrap_or(self.last_transform);
        (
            transform.map(|transform| {
                let mut objects = TransformedObjects::reflection(
                    self.sources,
                    !self.copy || matches!(self.plane, MirrorPointPlane::ThreePoint { .. }),
                    transform,
                );
                objects.grips = self.grips;
                objects.copy = self.copy;
                objects
            }),
            update,
        )
    }
}

impl Viewport {
    pub(super) fn resolve_mirror_preview<'a>(
        &self,
        preview: MirrorPreview<'a>,
        cursor: Option<Point3>,
        document: &Document,
    ) -> (
        Option<TransformedObjects<'a>>,
        Option<Option<AffineTransform3>>,
    ) {
        let resolved = preview.resolve(cursor, self.construction_plane(), document.tolerance());
        if let Some(objects) = resolved.0
            && resolved.1.is_some()
        {
            let owners = objects
                .grips
                .iter()
                .map(|id| id.object)
                .collect::<std::collections::BTreeSet<_>>();
            if !owners.iter().all(|id| {
                document.object(*id).is_some_and(|object| {
                    self.grip_preview_geometry(object, objects, document.tolerance())
                        .is_some()
                })
            }) {
                return (
                    preview
                        .resolve(None, self.construction_plane(), document.tolerance())
                        .0,
                    Some(preview.last_transform),
                );
            }
        }
        resolved
    }
}

#[cfg(test)]
mod tests;
