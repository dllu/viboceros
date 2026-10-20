//! Cached deformation cages for Stretch, including selected grip owners.
use super::display_cache::DisplayCache;
use super::morph_preview::{MorphPreviewCache, MorphPreviewImage, MorphPreviewKey};
use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use viboceros_command::stretch::{Options, Target};
use viboceros_document::ControlPointId;
use viboceros_geometry::{AffineTransform3, GeometryError, PointMorph, StretchPointMorph};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Key {
    start: Point3,
    end: Point3,
    target: Target,
    options: Options,
}
impl MorphPreviewKey for Key {
    type Morph = StretchPointMorph;
    fn morph(&self, _: Tolerance) -> Result<Self::Morph, GeometryError> {
        self.target.morph(self.start, self.end)
    }
    fn rigid(&self) -> bool {
        self.options.rigid
    }
    fn preserve(&self) -> bool {
        false
    }
    fn rigid_transform(
        &self,
        morph: &Self::Morph,
        center: Point3,
    ) -> Result<AffineTransform3, GeometryError> {
        morph.rigid_transform(center)
    }
}
pub(crate) type StretchPreviewCache = MorphPreviewCache<Key>;
#[derive(Clone, Copy)]
pub(crate) struct StretchPreview<'a> {
    pub sources: &'a [ObjectId],
    pub grips: &'a [ControlPointId],
    pub start: Point3,
    pub end: Point3,
    pub options: Options,
    pub last_point: Option<Point3>,
    pub cache: &'a RefCell<StretchPreviewCache>,
}
impl std::fmt::Debug for StretchPreview<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StretchPreview")
            .field("start", &self.start)
            .field("end", &self.end)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}
impl StretchPreview<'_> {
    pub(super) fn mouse_line(
        self,
    ) -> Option<viboceros_command::translation::DestinationConstraint> {
        Some(viboceros_command::translation::DestinationConstraint {
            anchor: self.start,
            direction: Some(self.start.direction_to(self.end).ok()?),
            distance: None,
        })
    }
    pub(super) fn resolve(
        self,
        cursor: Option<Point3>,
        doc: &Document,
        display: &RefCell<DisplayCache>,
    ) -> (Option<Rc<MorphPreviewImage>>, Option<Option<Point3>>) {
        let Some(target) = cursor.or(self.last_point).map(Target::Point) else {
            return (None, cursor.map(Some));
        };
        let key = Key {
            start: self.start,
            end: self.end,
            target,
            options: self.options,
        };
        match self
            .cache
            .borrow_mut()
            .get_with_grips(self.sources, self.grips, key, doc, display)
        {
            Ok(image) => (Some(image), cursor.map(Some)),
            Err(_) => (None, cursor.map(|_| None)),
        }
    }
}
impl Viewport {
    pub(super) fn paint_stretch_guide(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        preview: StretchPreview<'_>,
        cursor: Option<Point3>,
    ) {
        let mut segments = vec![[preview.start, preview.end]];
        if let Some(target) = cursor.or(preview.last_point).map(Target::Point)
            && let Ok(morph) = target.morph(preview.start, preview.end)
            && let Ok(point) = morph.morph_point(preview.end)
        {
            segments.push([preview.end, point]);
        }
        for [a, b] in segments {
            if let Some([a, b]) = self.project_segment(a, b, rect) {
                painter.line_segment([a, b], Stroke::new(1., Color32::BLACK));
            }
        }
    }
}
