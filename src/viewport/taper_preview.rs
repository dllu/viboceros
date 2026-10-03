//! Taper prepared display cages, leaving fitted model geometry untouched.
use super::display_cache::DisplayCache;
use super::morph_preview::{MorphPreviewCache, MorphPreviewImage, MorphPreviewKey};
use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use viboceros_command::taper::{TaperDistance, TaperOptions, point_morph};
use viboceros_geometry::{AffineTransform3, GeometryError, TaperPointMorph};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Key {
    start: Point3,
    end: Point3,
    initial: TaperDistance,
    target: Point3,
    options: TaperOptions,
    cplane: Frame3,
}
impl MorphPreviewKey for Key {
    type Morph = TaperPointMorph;
    fn morph(self, _: Tolerance) -> Result<Self::Morph, GeometryError> {
        point_morph(
            self.start,
            self.end,
            self.initial,
            TaperDistance::Point(self.target),
            self.options,
            viboceros_command::CommandContext {
                construction_plane: self.cplane,
            },
        )
    }
    fn rigid(self) -> bool {
        self.options.rigid
    }
    fn preserve(self) -> bool {
        self.options.preserve_structure
    }
    fn rigid_transform(
        self,
        morph: &Self::Morph,
        center: Point3,
    ) -> Result<AffineTransform3, GeometryError> {
        morph.rigid_transform(center)
    }
}
pub(crate) type TaperPreviewCache = MorphPreviewCache<Key>;

#[derive(Clone, Copy)]
pub(crate) struct TaperPreview<'a> {
    pub sources: &'a [ObjectId],
    pub start: Point3,
    pub end: Point3,
    pub initial: Option<TaperDistance>,
    pub options: TaperOptions,
    pub cplane: Frame3,
    pub last_point: Option<Point3>,
    pub cache: &'a RefCell<TaperPreviewCache>,
}
impl std::fmt::Debug for TaperPreview<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaperPreview")
            .field("sources", &self.sources)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("initial", &self.initial)
            .field("options", &self.options)
            .field("last_point", &self.last_point)
            .finish_non_exhaustive()
    }
}
impl TaperPreview<'_> {
    pub(super) fn anchor(self) -> Point3 {
        if self.initial.is_some() {
            self.end
        } else {
            self.start
        }
    }
    pub(super) fn mouse_plane(self) -> Option<(Point3, viboceros_geometry::UnitVector3)> {
        Some((
            self.anchor(),
            self.start
                .vector_to(self.end)
                .ok()?
                .normalized_nonzero()
                .ok()?,
        ))
    }
    pub(super) fn radius_point(self, point: Point3) -> Option<Point3> {
        let frame = Frame3::try_from_normal(
            self.anchor(),
            self.start.vector_to(self.end).ok()?,
            Tolerance::NUMERICAL_VALIDATION,
        )
        .ok()?;
        let [x, y] = frame.projected_coordinates_of(point).ok()?;
        frame.point_at([x, y, 0.]).ok()
    }
    pub(super) fn resolve(
        self,
        cursor: Option<Point3>,
        document: &Document,
        display_cache: &RefCell<DisplayCache>,
    ) -> (Option<Rc<MorphPreviewImage>>, Option<Option<Point3>>) {
        let Some(initial) = self.initial else {
            return (None, cursor.map(Some));
        };
        let Some(target) = cursor.or(self.last_point) else {
            return (None, None);
        };
        let options = TaperOptions {
            copy: false,
            preserve_structure: !self.options.rigid
                && self.options.preserve_structure
                && self.sources.iter().any(|id| {
                    document.object(*id).is_some_and(|o| {
                        matches!(o.geometry(), Geometry::NurbsSurface(_))
                            || matches!(o.geometry(),Geometry::Brep(b) if b.faces().len()==1)
                    })
                }),
            ..self.options
        };
        let key = Key {
            start: self.start,
            end: self.end,
            initial,
            target,
            options,
            cplane: self.cplane,
        };
        match self
            .cache
            .borrow_mut()
            .get(self.sources, key, document, display_cache)
        {
            Ok(image) => (Some(image), cursor.map(Some)),
            // Native Taper clears its temporary geometry at zero radius.
            Err(_) => (None, cursor.map(|_| None)),
        }
    }
}
impl Viewport {
    pub(super) fn paint_taper_guide(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        preview: TaperPreview<'_>,
        cursor: Option<Point3>,
    ) {
        let mut segments = vec![[preview.start, preview.end]];
        if let Some(point) = cursor
            .or(preview.last_point)
            .and_then(|p| preview.radius_point(p))
        {
            segments.push([preview.anchor(), point]);
        }
        for [a, b] in segments {
            if let Some(segment) = self.project_segment(a, b, rect) {
                painter.line_segment(segment, Stroke::new(1., Color32::BLACK));
            }
        }
        for point in [preview.start, preview.end] {
            if let Some(pixel) = self.project(point, rect) {
                painter.circle_filled(pixel, 3.5, Color32::WHITE);
                painter.circle_stroke(pixel, 3.5, Stroke::new(1., Color32::BLACK));
            }
        }
    }
}

#[cfg(test)]
mod tests;
