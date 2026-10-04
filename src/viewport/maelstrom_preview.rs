//! Cached Maelstrom wire cages and Circle guides without model fitting.
use super::display_cache::DisplayCache;
use super::morph_preview::{MorphPreviewCache, MorphPreviewImage, MorphPreviewKey};
use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use viboceros_command::maelstrom::{
    MaelstromOptions, MaelstromRadius, circle_frame, coil_angle, point_morph,
};
use viboceros_geometry::{AffineTransform3, GeometryError, MaelstromPointMorph};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MaelstromCursor {
    pub point: Point3,
    pub angle: Option<Real>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Key {
    center: Point3,
    initial: MaelstromRadius,
    target: MaelstromRadius,
    degrees: Real,
    rigid: bool,
    cplane: Frame3,
}
impl MorphPreviewKey for Key {
    type Morph = MaelstromPointMorph;
    fn morph(self, _: Tolerance) -> Result<Self::Morph, GeometryError> {
        point_morph(
            self.center,
            self.initial,
            self.target,
            self.degrees,
            viboceros_command::CommandContext {
                construction_plane: self.cplane,
            },
        )
    }
    fn rigid(self) -> bool {
        self.rigid
    }
    fn preserve(self) -> bool {
        false
    }
    fn rigid_transform(
        self,
        morph: &Self::Morph,
        center: Point3,
    ) -> Result<AffineTransform3, GeometryError> {
        morph.rigid_transform(center)
    }
}
pub(crate) type MaelstromPreviewCache = MorphPreviewCache<Key>;

#[derive(Clone, Copy)]
pub(crate) struct MaelstromPreview<'a> {
    pub circle_getter: Option<viboceros_command::circle_input::CircleInput>,
    pub sources: &'a [ObjectId],
    pub center: Point3,
    pub initial: Option<MaelstromRadius>,
    pub target: Option<MaelstromRadius>,
    pub options: MaelstromOptions,
    pub cplane: Frame3,
    pub last: Option<MaelstromCursor>,
    pub cache: &'a RefCell<MaelstromPreviewCache>,
}
impl std::fmt::Debug for MaelstromPreview<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaelstromPreview")
            .field("sources", &self.sources)
            .field("center", &self.center)
            .field("initial", &self.initial)
            .field("target", &self.target)
            .field("options", &self.options)
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}
impl MaelstromPreview<'_> {
    pub(super) fn frame(self) -> Option<Frame3> {
        circle_frame(
            self.center,
            self.initial.unwrap_or(MaelstromRadius::Number(1.)),
            viboceros_command::CommandContext {
                construction_plane: self.cplane,
            },
        )
        .ok()
    }
    pub(crate) fn angle(self, point: Point3) -> Option<Real> {
        let frame = self.frame()?;
        let previous = self.last.and_then(|c| c.angle);
        let principal = coil_angle(frame, point).ok()?;
        Some(super::twist_preview::continuous_angle(
            principal,
            previous.unwrap_or(0.),
        ))
    }
    pub(super) fn resolve(
        self,
        cursor: Option<Point3>,
        document: &Document,
        display: &RefCell<DisplayCache>,
    ) -> (Option<Rc<MorphPreviewImage>>, Option<MaelstromCursor>) {
        let update = cursor.map(|point| MaelstromCursor {
            point,
            angle: self.target.and_then(|_| self.angle(point)),
        });
        let last = update.or(self.last);
        let (Some(initial), Some(target), Some(degrees)) =
            (self.initial, self.target, last.and_then(|c| c.angle))
        else {
            return (None, update);
        };
        // An axial pick still resolves an accepted GetAngle value, including
        // accumulated turns, but native dynamic drawing hides its deformation.
        if self
            .frame()
            .and_then(|frame| frame.projected_coordinates_of(last.unwrap().point).ok())
            .is_some_and(|[x, y]| x == 0. && y == 0.)
        {
            return (None, update);
        }
        let key = Key {
            center: self.center,
            initial,
            target,
            degrees,
            rigid: self.options.rigid,
            cplane: self.cplane,
        };
        let mut cache = self.cache.borrow_mut();
        let image = cache
            .get(self.sources, key, document, display)
            .ok()
            .or_else(|| cache.image.clone());
        (image, update)
    }
    /// Native radius guides use full distance at the first Circle getter and
    /// radial projection at the second. Both accepted circles stay during GetAngle.
    pub(super) fn circles(self, cursor: Option<Point3>) -> Vec<(Frame3, Real)> {
        let point = cursor.or(self.last.map(|c| c.point));
        if let Some(getter) = self.circle_getter {
            return point
                .and_then(|point| getter.preview(point))
                .map(|circle| (circle.frame, circle.radius))
                .into_iter()
                .collect();
        }
        let context = viboceros_command::CommandContext {
            construction_plane: self.cplane,
        };
        let Some(initial) = self.initial else {
            return point
                .and_then(|p| {
                    let radius = MaelstromRadius::Point(p);
                    let frame = circle_frame(self.center, radius, context).ok()?;
                    Some((frame, radius.radius(frame).ok()?))
                })
                .into_iter()
                .collect();
        };
        let Some(frame) = self.frame() else {
            return Vec::new();
        };
        let mut result = initial
            .radius(frame)
            .ok()
            .map(|r| (frame, r))
            .into_iter()
            .collect::<Vec<_>>();
        let target = self.target.or(point.map(MaelstromRadius::Point));
        if let Some(target) = target {
            let radius = match target {
                MaelstromRadius::Number(r) => Some(r.abs()),
                MaelstromRadius::Point(p) => frame
                    .projected_coordinates_of(p)
                    .ok()
                    .map(|[x, y]| x.hypot(y)),
            };
            if let Some(r) = radius {
                result.push((frame, r));
            }
        }
        result
    }
}

impl Viewport {
    pub(super) fn paint_maelstrom_guide(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        preview: MaelstromPreview<'_>,
        cursor: Option<Point3>,
    ) {
        for (frame, radius) in preview.circles(cursor) {
            if !radius.is_finite() || radius <= 0. {
                continue;
            }
            let points = (0..=128)
                .filter_map(|i| {
                    let angle = std::f64::consts::TAU * i as Real / 128.;
                    frame
                        .point_at([radius * angle.cos(), radius * angle.sin(), 0.])
                        .ok()
                })
                .collect::<Vec<_>>();
            for pair in points.windows(2) {
                if let Some(line) = self.project_segment(pair[0], pair[1], rect) {
                    painter.line_segment(line, Stroke::new(1., Color32::BLACK));
                }
            }
        }
        let marker = preview
            .initial
            .and_then(|r| {
                let frame = preview.frame()?;
                frame.point_at([r.radius(frame).ok()?, 0., 0.]).ok()
            })
            .unwrap_or(preview.center);
        if let Some(pixel) = self.project(marker, rect) {
            painter.circle_filled(pixel, 3.5, Color32::WHITE);
            painter.circle_stroke(pixel, 3.5, Stroke::new(1., Color32::BLACK));
        }
    }
}

#[cfg(test)]
mod tests;
