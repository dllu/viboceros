//! Circular bending of prepared display cages without changing model geometry.
use super::display_cache::DisplayCache;
use super::morph_preview::{MorphPreviewCache, MorphPreviewImage, MorphPreviewKey};
use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use viboceros_command::bend::BendOptions;
use viboceros_geometry::{AffineTransform3, BendPointMorph, PointMorph};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Key {
    start: Point3,
    end: Point3,
    through: Point3,
    angle: Option<Real>,
    rigid: bool,
    limited: bool,
    symmetric: bool,
    preserve: bool,
    uniform: bool,
}

impl MorphPreviewKey for Key {
    type Morph = BendPointMorph;
    fn morph(self, tolerance: Tolerance) -> Result<Self::Morph, viboceros_geometry::GeometryError> {
        Ok(BendPointMorph::try_for_command(
            self.start,
            self.end,
            self.through,
            self.angle.map(Real::to_radians),
            self.limited,
            self.symmetric,
            tolerance,
        )?
        .with_non_attenuated(self.uniform))
    }
    fn rigid(self) -> bool {
        self.rigid
    }
    fn preserve(self) -> bool {
        self.preserve
    }
    fn rigid_transform(
        self,
        morph: &Self::Morph,
        center: Point3,
    ) -> Result<AffineTransform3, viboceros_geometry::GeometryError> {
        morph.rigid_transform(center)
    }
    fn guide(self, morph: &Self::Morph) -> Result<Vec<Point3>, viboceros_geometry::GeometryError> {
        let axis = self
            .start
            .vector_to(self.end)?
            .normalized_nonzero()?
            .as_vector();
        let intervals = (morph.angle() / std::f64::consts::TAU * 256.)
            .ceil()
            .clamp(1., 256.) as usize;
        let mut points =
            (0..=intervals)
                .map(|i| {
                    morph.morph_point(self.start.translated(
                        axis.scaled(morph.arc_length() * i as Real / intervals as Real)?,
                    )?)
                })
                .collect::<Result<Vec<_>, _>>()?;
        if self.angle.is_none_or(|a| a == 0.) {
            let end = *points.last().unwrap();
            let beyond = morph.morph_point(
                self.start
                    .translated(axis.scaled(morph.arc_length() + morph.radius().max(1.))?)?,
            )?;
            if end.vector_to(self.through)?.dot(end.vector_to(beyond)?)? > 0. {
                points.push(self.through);
            }
        }
        Ok(points)
    }
    fn omit_shaded_isocurves(self) -> bool {
        true
    }
}

impl Viewport {
    pub(super) fn paint_bend_guide(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        preview: BendPreview<'_>,
        image: Option<&MorphPreviewImage>,
    ) {
        if let Some(image) = image {
            for pair in image.guide.windows(2) {
                if let Some(segment) = self.project_segment(pair[0], pair[1], rect) {
                    painter.line_segment(segment, Stroke::new(1., Color32::BLACK));
                }
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

pub(crate) type BendPreviewCache = MorphPreviewCache<Key>;

#[derive(Clone, Copy)]
pub(crate) struct BendPreview<'a> {
    pub sources: &'a [ObjectId],
    pub start: Point3,
    pub end: Point3,
    /// Effective numeric angle, including the remembered command default.
    pub options: BendOptions,
    pub last_point: Option<Point3>,
    pub cache: &'a RefCell<BendPreviewCache>,
}

impl std::fmt::Debug for BendPreview<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BendPreview")
            .field("sources", &self.sources)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("options", &self.options)
            .field("last_point", &self.last_point)
            .finish_non_exhaustive()
    }
}

impl BendPreview<'_> {
    pub(super) fn resolve(
        self,
        cursor: Option<Point3>,
        document: &Document,
        display_cache: &RefCell<DisplayCache>,
    ) -> (Option<Rc<MorphPreviewImage>>, Option<Option<Point3>>) {
        let mut cache = self.cache.borrow_mut();
        let point = cursor.or(self.last_point);
        let Some(through) = point else {
            return (None, None);
        };
        let key = Key {
            start: self.start,
            end: self.end,
            through,
            angle: self.options.angle,
            rigid: self.options.rigid,
            limited: self.options.limit_to_spine,
            symmetric: self.options.symmetric,
            preserve: !self.options.rigid
                && self.options.preserve_structure
                && self.sources.iter().any(|id| {
                    document.object(*id).is_some_and(|o| {
                        matches!(o.geometry(), Geometry::NurbsSurface(_))
                            || matches!(o.geometry(), Geometry::Brep(b) if b.faces().len()==1)
                    })
                }),
            uniform: self.options.non_attenuated,
        };
        match cache.get(self.sources, key, document, display_cache) {
            Ok(image) => (Some(image), cursor.map(Some)),
            // Unlike Twist's angle prompt, native Bend removes its temporary
            // deformation when the through point is on the spine.
            Err(_) => (None, cursor.map(|_| None)),
        }
    }
}

#[cfg(test)]
mod tests;
