//! Flow uses the existing shared display cages and prepared guide mapping.
use super::display_cache::DisplayCache;
use super::morph_preview::{MorphPreviewCache, MorphPreviewImage, MorphPreviewKey};
use super::*;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use viboceros_command::flow::Options;
use viboceros_geometry::{AffineTransform3, CurveFlowPointMorph, GeometryError};
#[derive(Clone)]
pub(crate) struct Key {
    morph: Arc<CurveFlowPointMorph>,
    options: Options,
}
impl PartialEq for Key {
    fn eq(&self, rhs: &Self) -> bool {
        Arc::ptr_eq(&self.morph, &rhs.morph)
            && self.options.rigid == rhs.options.rigid
            && self.options.rigid_groups == rhs.options.rigid_groups
    }
}
impl MorphPreviewKey for Key {
    type Morph = CurveFlowPointMorph;
    fn morph(&self, _: Tolerance) -> Result<Self::Morph, GeometryError> {
        Ok((*self.morph).clone())
    }
    fn rigid(&self) -> bool {
        self.options.rigid
    }
    fn rigid_groups(&self) -> bool {
        self.options.rigid_groups
    }
    fn preserve(&self) -> bool {
        self.options.preserve_structure
    }
    fn rigid_transform(
        &self,
        morph: &Self::Morph,
        center: Point3,
    ) -> Result<AffineTransform3, GeometryError> {
        morph.rigid_transform(center)
    }
}
pub(crate) type FlowPreviewCache = MorphPreviewCache<Key>;
#[derive(Clone, Copy)]
pub(crate) struct FlowPreview<'a> {
    pub sources: &'a [ObjectId],
    pub morph: &'a Arc<CurveFlowPointMorph>,
    pub options: Options,
    pub cache: &'a RefCell<FlowPreviewCache>,
}
impl std::fmt::Debug for FlowPreview<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlowPreview")
            .field("sources", &self.sources)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}
impl FlowPreview<'_> {
    pub(super) fn resolve(
        self,
        doc: &Document,
        display: &RefCell<DisplayCache>,
    ) -> Option<Rc<MorphPreviewImage>> {
        self.cache
            .borrow_mut()
            .get(
                self.sources,
                Key {
                    morph: self.morph.clone(),
                    options: self.options,
                },
                doc,
                display,
            )
            .ok()
    }
}
impl Viewport {
    pub(crate) fn flow_pick_reversed(
        &self,
        geometry: &Geometry,
        edge: Option<usize>,
        pointer: Pos2,
        tolerance: Tolerance,
    ) -> Option<bool> {
        let guide = viboceros_command::curve_reference::resolve(geometry, edge, tolerance)?;
        let curve = guide.curve();
        let rect = self.last_rect?;
        let a = self.project(curve.start_point().ok()?, rect)?;
        let b = self.project(curve.end_point().ok()?, rect)?;
        Some(pointer.distance_sq(b) < pointer.distance_sq(a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::LineSegment;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    #[test]
    fn flow_preview_is_shared_readonly_and_rigid_groups_follow_command_policy() {
        let mut d = Document::default();
        let a = d
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(10., 0., 0.), d.tolerance()).unwrap(),
            ))
            .unwrap();
        let b = d
            .add_geometry(Geometry::Arc(
                viboceros_geometry::CircularArc3::try_from_circle_sweep(
                    viboceros_geometry::Circle3::try_new(
                        p(0., 0., 0.),
                        10.,
                        viboceros_geometry::Vector3::try_new(0., 0., 1.)
                            .unwrap()
                            .normalized_nonzero()
                            .unwrap(),
                        d.tolerance(),
                    )
                    .unwrap(),
                    std::f64::consts::FRAC_PI_2,
                )
                .unwrap(),
            ))
            .unwrap();
        let ids =
            [p(2., 1., 0.), p(7., 2., 1.)].map(|p| d.add_geometry(Geometry::Point(p)).unwrap());
        d.add_group(None, ids).unwrap();
        d.clear_history().unwrap();
        let before = format!("{d:?}");
        let display = RefCell::new(DisplayCache::default());
        let cache = RefCell::new(FlowPreviewCache::default());
        for groups in [false, true] {
            let options = Options {
                rigid: true,
                rigid_groups: groups,
                stretch: true,
                ..Default::default()
            };
            let morph = Arc::new(
                viboceros_command::flow::prepare(
                    &d,
                    viboceros_command::flow::GuideReference { id: a, edge: None },
                    viboceros_command::flow::GuideReference { id: b, edge: None },
                    options,
                )
                .unwrap(),
            );
            let preview = FlowPreview {
                sources: &ids,
                morph: &morph,
                options,
                cache: &cache,
            };
            let image = preview.resolve(&d, &display).unwrap();
            assert!(Rc::ptr_eq(&image, &preview.resolve(&d, &display).unwrap()));
            let expected =
                viboceros_command::flow::deformed_geometries(&d, &ids, &morph, options).unwrap();
            for (id, g) in expected {
                let object = &image.objects[&id];
                let actual = object
                    .geometry
                    .geometry
                    .transformed(object.transform.unwrap(), d.tolerance())
                    .unwrap();
                assert_eq!(actual, g);
            }
        }
        assert_eq!(format!("{d:?}"), before);
    }
    #[test]
    fn flow_curve_pick_uses_the_nearer_projected_end() {
        let mut v = Viewport::new(ViewKind::Top);
        v.last_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.)));
        let g = Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(10., 0., 0.), Tolerance::default()).unwrap(),
        );
        for (point, reverse) in [(p(0., 0., 0.), false), (p(10., 0., 0.), true)] {
            let pointer = v.project(point, v.last_rect.unwrap()).unwrap();
            assert_eq!(
                v.flow_pick_reversed(&g, None, pointer, Tolerance::default()),
                Some(reverse)
            );
        }
    }
}
