//! Rigid display instances share original geometry, samples, and tessellation.
use super::*;
use viboceros_geometry::AffineTransform3;

#[derive(Clone, Copy, Debug)]
pub(super) struct TransformedObjects<'a> {
    pub sources: &'a [ObjectId],
    pub reference_sources: bool,
    pub draw_source_faces: bool,
    pub reversing: bool,
    /// A Normal reference uses temporary selection styling without selecting it in the model.
    pub reference: Option<ObjectId>,
    pub transform: AffineTransform3,
}

impl<'a> TransformedObjects<'a> {
    pub(super) fn reflection(
        sources: &'a [ObjectId],
        reference_sources: bool,
        transform: AffineTransform3,
    ) -> Self {
        Self {
            sources,
            reference_sources,
            draw_source_faces: false,
            reversing: true,
            reference: None,
            transform,
        }
    }
}
