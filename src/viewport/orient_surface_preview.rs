//! Surface placement reuses the shared, immutable deformation display cages.
use super::display_cache::DisplayCache;
use super::morph_preview::{MorphPreviewCache, MorphPreviewImage, MorphPreviewKey};
use super::*;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use viboceros_command::orient_surface::{Options, Placement, Pose, PreviewPhase};
use viboceros_geometry::{AffineTransform3, GeometryError, SurfacePointMorph};
#[derive(Clone)]
pub(crate) struct Key {
    placement: Placement,
    pose: Pose,
    options: Options,
    normal: Vector3,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(
            &self.placement.target.surface,
            &other.placement.target.surface,
        ) && self.placement.source == other.placement.source
            && self.pose == other.pose
            && self.options == other.options
            && self.normal == other.normal
    }
}
impl MorphPreviewKey for Key {
    type Morph = SurfacePointMorph<'static>;
    fn morph(&self, _: Tolerance) -> Result<Self::Morph, GeometryError> {
        self.placement
            .morph_at_pose(self.pose, self.options, self.normal)
            .map_err(|_| GeometryError::Degenerate {
                context: "surface orientation preview",
            })
    }
    fn rigid(&self) -> bool {
        false
    }
    fn preserve(&self) -> bool {
        false
    }
    fn rigid_transform(
        &self,
        _: &Self::Morph,
        _: Point3,
    ) -> Result<AffineTransform3, GeometryError> {
        self.placement
            .transform_at_pose(self.pose, self.options)
            .map_err(|_| GeometryError::Degenerate {
                context: "surface orientation preview",
            })
    }
    fn omit_shaded_isocurves(&self) -> bool {
        true
    }
}
pub(crate) type OrientSurfacePreviewCache = MorphPreviewCache<Key>;
#[derive(Clone, Copy)]
pub(crate) struct OrientSurfacePreview<'a> {
    pub sources: &'a [ObjectId],
    pub placement: &'a Placement,
    pub phase: PreviewPhase,
    pub normal: Vector3,
    pub last_normal: Vector3,
    pub last_point: Option<Point3>,
    pub cache: &'a RefCell<OrientSurfacePreviewCache>,
}
impl std::fmt::Debug for OrientSurfacePreview<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrientSurfacePreview")
            .field("sources", &self.sources)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}
impl OrientSurfacePreview<'_> {
    pub(super) fn resolve(
        self,
        cursor: Option<Point3>,
        document: &Document,
        display_cache: &RefCell<DisplayCache>,
    ) -> (Option<Rc<MorphPreviewImage>>, Option<Option<Point3>>) {
        let Some(point) = cursor.or(self.last_point) else {
            return (None, None);
        };
        let image = self
            .placement
            .preview_pose(self.phase, point)
            .and_then(|(pose, options)| {
                self.cache.borrow_mut().get(
                    self.sources,
                    Key {
                        placement: self.placement.clone(),
                        pose,
                        options,
                        normal: if cursor.is_some() {
                            self.normal
                        } else {
                            self.last_normal
                        },
                    },
                    document,
                    display_cache,
                )
            });
        match image {
            Ok(image) => (Some(image), cursor.map(Some)),
            Err(_) => (None, cursor.map(|_| None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::preview_test_support::frame;
    use viboceros_oracle::OrientSurfaceFixture;
    fn fixture() -> OrientSurfaceFixture {
        let v: serde_json::Value = serde_json::from_str(include_str!(
            "../../tools/rhino_oracle/fixtures/orient_surface_policy.json"
        ))
        .unwrap();
        serde_json::from_value(v["operations"].as_array().unwrap().last().unwrap().clone()).unwrap()
    }
    fn setup() -> (Document, ObjectId, Placement, Point3) {
        let f = fixture();
        let mut d = Document::default();
        let guide = d.add_geometry(f.guide(d.tolerance()).unwrap()).unwrap();
        let source = d
            .add_geometry(Geometry::PointCloud(f.source().unwrap()))
            .unwrap();
        let placement = Placement::try_new(
            Point3::try_from(f.base).unwrap(),
            Point3::try_from(f.reference).unwrap(),
            viboceros_command::orient_surface::Target::capture(&d, guide, None).unwrap(),
            f.options.options(),
            f.plane(d.tolerance()).unwrap(),
            d.tolerance(),
        )
        .unwrap();
        let q = f.target(f.uv, d.tolerance()).unwrap();
        d.clear_history().unwrap();
        (d, source, placement, q)
    }
    #[test]
    fn orient_surface_preview_shares_images_and_invalidates_on_source_edits() {
        let (mut doc, id, placement, q) = setup();
        let ids = [id];
        let cache = RefCell::new(OrientSurfacePreviewCache::default());
        let display = RefCell::new(DisplayCache::default());
        let normal = Vector3::try_new(0., 0., 1.).unwrap();
        let p = OrientSurfacePreview {
            sources: &ids,
            placement: &placement,
            phase: PreviewPhase::Target,
            normal,
            last_normal: normal,
            last_point: Some(q),
            cache: &cache,
        };
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let image = p.resolve(Some(q), &doc, &display).0.unwrap();
        for _ in 0..4 {
            assert!(Rc::ptr_eq(
                &image,
                &p.resolve(None, &doc, &display).0.unwrap()
            ));
        }
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.undo_label().is_none());
        let map = AffineTransform3::from_translation(Vector3::try_new(1., 0., 0.).unwrap());
        doc.transform_objects([id], map).unwrap();
        assert!(!Rc::ptr_eq(
            &image,
            &p.resolve(None, &doc, &display).0.unwrap()
        ));
        let tangent = Vector3::try_new(10.8, 0., 3.2).unwrap();
        let invalid = OrientSurfacePreview {
            normal: tangent,
            ..p
        };
        assert!(invalid.resolve(Some(q), &doc, &display).0.is_none());
    }
    #[test]
    fn orient_surface_pointer_targets_and_deformed_previews_work_in_all_views() {
        for kind in [
            ViewKind::Top,
            ViewKind::Front,
            ViewKind::Right,
            ViewKind::Perspective,
        ] {
            for mode in [
                DisplayMode::Wireframe,
                DisplayMode::Shaded,
                DisplayMode::Ghosted,
            ] {
                let (doc, id, placement, q) = setup();
                let ids = [id];
                let cache = RefCell::new(OrientSurfacePreviewCache::default());
                let ctx = egui::Context::default();
                let mut view = Viewport::new(kind);
                view.display_mode = mode;
                // A fixed normal isolates target picking from the separate per-view normal policy.
                let normal = Vector3::try_new(0., 0., 1.).unwrap();
                let preview = OrientSurfacePreview {
                    sources: &ids,
                    placement: &placement,
                    phase: PreviewPhase::Target,
                    normal,
                    last_normal: normal,
                    last_point: None,
                    cache: &cache,
                };
                let input = ViewportInput {
                    drafting: DraftingInput {
                        active: true,
                        ..Default::default()
                    },
                    normal_surface: Some((placement.target.id, Some(0), false)),
                    orient_surface_preview: Some(preview),
                    ..Default::default()
                };
                let before = doc.objects().cloned().collect::<Vec<_>>();
                frame(&ctx, &mut view, &doc, input, vec![]);
                let pointer = view.project(q, view.last_rect.unwrap()).unwrap();
                let events = vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::PointerButton {
                        pos: pointer,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                    egui::Event::PointerButton {
                        pos: pointer,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers: Default::default(),
                    },
                ];
                let out = frame(&ctx, &mut view, &doc, input, events);
                assert!(
                    out.picked_point.unwrap().distance_to(q).unwrap() < 1e-4,
                    "{kind:?} {mode:?}"
                );
                assert!(
                    out.orient_surface_preview
                        .flatten()
                        .unwrap()
                        .distance_to(q)
                        .unwrap()
                        < 1e-4
                );
                assert!(out.face_click.is_none());
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
                assert!(doc.undo_label().is_none());
            }
        }
    }
}
