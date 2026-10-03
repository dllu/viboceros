//! Mirror previews apply reflections to cached display samples, never the document.
use super::object_preview::TransformedObjects;
use super::*;
use viboceros_command::mirror::MirrorPointPlane;
use viboceros_geometry::AffineTransform3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MirrorPreview<'a> {
    pub plane: MirrorPointPlane,
    pub sources: &'a [ObjectId],
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
                TransformedObjects::reflection(
                    self.sources,
                    !self.copy || matches!(self.plane, MirrorPointPlane::ThreePoint { .. }),
                    transform,
                )
            }),
            update,
        )
    }
}

#[cfg(test)]
mod tests;
