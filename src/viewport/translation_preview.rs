//! Move/Copy previews follow the resolved drafting point without document edits.
use super::object_preview::TransformedObjects;
use super::*;
use viboceros_geometry::AffineTransform3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TranslationPreview<'a> {
    pub sources: &'a [ObjectId],
    pub grips: &'a [viboceros_document::ControlPointId],
    pub base: Point3,
    pub copy: bool,
    pub reference: Option<ObjectId>,
    pub last_transform: Option<AffineTransform3>,
}

impl<'a> TranslationPreview<'a> {
    pub(super) fn resolve(
        self,
        cursor: Option<Point3>,
    ) -> (
        Option<TransformedObjects<'a>>,
        Option<Option<AffineTransform3>>,
    ) {
        let update = cursor.map(|point| {
            self.base
                .vector_to(point)
                .ok()
                .map(AffineTransform3::from_translation)
                .or(self.last_transform)
        });
        let transform = update.unwrap_or(self.last_transform);
        (
            transform.map(|transform| TransformedObjects {
                rigid_layout: None,
                sources: self.sources,
                grips: self.grips,
                copy: self.copy,
                reference_sources: !self.copy,
                draw_source_faces: false,
                reversing: false,
                reference: self.reference,
                transform,
            }),
            update,
        )
    }
}

impl Viewport {
    pub(super) fn resolve_translation_preview<'a>(
        &self,
        preview: TranslationPreview<'a>,
        cursor: Option<Point3>,
        document: &Document,
    ) -> (
        Option<TransformedObjects<'a>>,
        Option<Option<AffineTransform3>>,
    ) {
        let resolved = preview.resolve(cursor);
        if let Some(objects) = resolved.0
            && resolved.1.is_some()
        {
            let mut cache = self.display_cache.borrow_mut();
            let valid = objects
                .grips
                .iter()
                .map(|id| id.object)
                .collect::<std::collections::BTreeSet<_>>()
                .iter()
                .all(|id| {
                    document.object(*id).is_some_and(|object| {
                        self.grip_preview_geometry(object, objects, document.tolerance())
                            .is_some()
                    })
                })
                && objects
                    .sources
                    .iter()
                    .filter(|id| !objects.grips.iter().any(|p| p.object == **id))
                    .all(|id| {
                        let Some(object) = document.object(*id) else {
                            return false;
                        };
                        let bounds = cache.get(object, document.tolerance()).bounds();
                        objects.transform.transform_point(bounds.min()).is_ok()
                            && objects.transform.transform_point(bounds.max()).is_ok()
                    });
            if !valid {
                return (preview.resolve(None).0, Some(preview.last_transform));
            }
        }
        resolved
    }
}

#[cfg(test)]
pub(crate) mod tests;
