//! Tight geometry bounds in an explicitly chosen, geometry-local working frame.
use crate::CommandContext;
use std::borrow::Cow;
use viboceros_document::Geometry;
use viboceros_geometry::{AffineTransform3, BoundingBox3, Frame3, GeometryError, Tolerance};

/// Choose `frame.origin()` near the input geometry. A construction plane's
/// distant display origin need not be the working origin of a layout query.
pub(super) fn local_bounds<'a>(
    geometries: impl IntoIterator<Item = &'a Geometry>,
    frame: Frame3,
    tolerance: Tolerance,
) -> Result<BoundingBox3, GeometryError> {
    let world = CommandContext::default().construction_plane;
    let orientation = frame.with_origin(world.origin());
    let translation = AffineTransform3::from_translation(frame.origin().vector_to(world.origin())?);
    let rotation = AffineTransform3::try_frame_mapping(orientation, world, [1.; 3])?;
    let query = |geometry: &Geometry| -> Result<BoundingBox3, GeometryError> {
        // Subtract before rotating; a combined matrix can cancel large terms
        // only after their low-order coordinate differences have been lost.
        let translated = if frame.origin() == world.origin() {
            Cow::Borrowed(geometry)
        } else {
            Cow::Owned(geometry.transformed(translation, tolerance)?)
        };
        let local = if orientation == world {
            translated
        } else {
            Cow::Owned(translated.transformed(rotation, tolerance)?)
        };
        local.tight_bounds(tolerance)
    };
    let mut geometries = geometries.into_iter();
    let first = geometries.next().ok_or(GeometryError::EmptyPointSet)?;
    geometries.try_fold(query(first)?, |bounds, geometry| {
        bounds.union(query(geometry)?)
    })
}
