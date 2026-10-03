//! Prepared wire cages shared by all viewports; no model edits or morph fitting.
use super::display_cache::DisplayCache;
#[cfg(test)]
use super::morph_preview::Cage;
use super::morph_preview::{MorphPreviewCache, MorphPreviewImage, MorphPreviewKey};
use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use viboceros_command::twist::{TwistOptions, reference_angle};
use viboceros_geometry::{AffineTransform3, TwistPointMorph};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Key {
    start: Point3,
    end: Point3,
    angle: Real,
    rigid: bool,
    infinite: bool,
    preserve: bool,
}

impl MorphPreviewKey for Key {
    type Morph = TwistPointMorph;
    fn morph(self, tolerance: Tolerance) -> Result<Self::Morph, viboceros_geometry::GeometryError> {
        TwistPointMorph::try_new(
            self.start,
            self.end,
            self.angle.to_radians(),
            self.infinite,
            tolerance,
        )
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
}

pub(crate) type TwistPreviewCache = MorphPreviewCache<Key>;

#[derive(Clone, Copy)]
pub(crate) struct TwistPreview<'a> {
    pub sources: &'a [ObjectId],
    pub start: Point3,
    pub end: Point3,
    pub reference: Point3,
    pub options: TwistOptions,
    pub last_angle: Option<Real>,
    pub cache: &'a RefCell<TwistPreviewCache>,
}

impl std::fmt::Debug for TwistPreview<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TwistPreview")
            .field("sources", &self.sources)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("reference", &self.reference)
            .field("options", &self.options)
            .field("last_angle", &self.last_angle)
            .finish_non_exhaustive()
    }
}

/// Choose the continuous turn nearest the last mouse angle. Typed scalar input
/// keeps its explicit angle; only the reference-point mouse path accumulates turns.
pub(crate) fn continuous_angle(principal: Real, previous: Real) -> Real {
    principal + ((previous - principal) / 360.).round() * 360.
}

#[cfg(test)]
mod tests;

impl TwistPreview<'_> {
    pub(super) fn resolve(
        self,
        cursor: Option<Point3>,
        document: &Document,
        display_cache: &RefCell<DisplayCache>,
    ) -> (Option<Rc<MorphPreviewImage>>, Option<Option<Real>>) {
        let update = cursor.and_then(|p| {
            reference_angle(
                self.start,
                self.end,
                self.reference,
                p,
                document.tolerance(),
            )
            .ok()
            .map(|a| continuous_angle(a, self.last_angle.unwrap_or(0.)))
        });
        let angle = update.or(self.last_angle);
        let mut cache = self.cache.borrow_mut();
        let Some(angle) = angle else {
            return (None, None);
        };
        let key = Key {
            start: self.start,
            end: self.end,
            angle,
            rigid: self.options.rigid,
            infinite: self.options.infinite,
            preserve: !self.options.rigid
                && self.options.preserve_structure
                && self.sources.iter().any(|id| {
                    document.object(*id).is_some_and(|o| {
                        matches!(o.geometry(), Geometry::NurbsSurface(_))
                            || matches!(o.geometry(), Geometry::Brep(b) if b.faces().len()==1)
                    })
                }),
        };
        let image = cache.get(self.sources, key, document, display_cache).ok();
        // Native GetAngle retains the last valid deformation on an axial pick.
        // Failed preparation also retains the previous display without changing the model.
        let image = image.or_else(|| cache.image.clone());
        (image, update.map(Some))
    }
}
