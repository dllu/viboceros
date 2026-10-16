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
    pub grips: &'a [viboceros_document::ControlPointId],
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
                self.cache.borrow_mut().get_with_grips(
                    self.sources,
                    self.grips,
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
            grips: &[],
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
                    grips: &[],
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

#[cfg(test)]
mod grip_tests {
    use super::*;
    use viboceros_document::{ControlPointId, GripMode};
    use viboceros_oracle::OrientSurfaceGripsFixture;
    fn recipes() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../tools/rhino_oracle/fixtures/orient_surface_grips.json"
        ))
        .unwrap()
    }
    #[test]
    fn orient_surface_grip_previews_share_exact_partial_owner_edits_and_invalidate_modes_and_picks()
    {
        let rows = recipes();
        for id in [
            "rational-False-False",
            "periodic-False-False",
            "surface-False-False",
            "mesh-False-False",
            "edit-False-False",
        ] {
            let r = rows["operations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == id)
                .unwrap();
            let f: OrientSurfaceGripsFixture = serde_json::from_value(r.clone()).unwrap();
            let mut doc = Document::default();
            let guide = doc
                .add_geometry(f.placement.guide(doc.tolerance()).unwrap())
                .unwrap();
            let source = doc
                .add_geometry(f.source_geometry(doc.tolerance()).unwrap())
                .unwrap();
            match f.mode() {
                GripMode::Control => doc.enable_control_points([source]).unwrap(),
                GripMode::Edit => doc.enable_edit_points([source]).unwrap(),
            };
            let picks = f
                .selected
                .iter()
                .map(|&index| ControlPointId {
                    object: source,
                    index,
                })
                .collect::<Vec<_>>();
            let ids = [source];
            let target =
                viboceros_command::orient_surface::Target::capture(&doc, guide, None).unwrap();
            let placement = Placement::try_new(
                Point3::try_from(f.placement.base).unwrap(),
                Point3::try_from(f.placement.reference).unwrap(),
                target,
                f.placement.options.options(),
                f.placement.plane(doc.tolerance()).unwrap(),
                doc.tolerance(),
            )
            .unwrap();
            let q = f.placement.target(f.placement.uv, doc.tolerance()).unwrap();
            let normal = Vector3::try_new(0., 0., 1.).unwrap();
            let cache = RefCell::new(OrientSurfacePreviewCache::default());
            let display = RefCell::new(DisplayCache::default());
            let preview = OrientSurfacePreview {
                sources: &ids,
                grips: &picks,
                placement: &placement,
                phase: PreviewPhase::Target,
                normal,
                last_normal: normal,
                last_point: Some(q),
                cache: &cache,
            };
            let before = doc.objects().cloned().collect::<Vec<_>>();
            doc.clear_history().unwrap();
            let image = preview.resolve(None, &doc, &display).0.unwrap();
            for _ in 0..4 {
                assert!(Rc::ptr_eq(
                    &image,
                    &preview.resolve(None, &doc, &display).0.unwrap()
                ));
            }
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(doc.undo_label().is_none());
            assert_eq!(image.grips.len(), picks.len());
            let point = |p: [f64; 3]| p.map(|x| x.to_string()).join(",");
            let command = format!(
                "OrientOnSrf {} {} {} Surface={guide} Sources={source} GripSources={} {}",
                point(f.placement.base),
                point(f.placement.reference),
                point(q.to_array()),
                picks
                    .iter()
                    .map(|g| format!("{}:{}", g.object, g.index))
                    .collect::<Vec<_>>()
                    .join(","),
                f.placement.options.options().arguments()
            );
            viboceros_command::CommandRegistry::with_builtins()
                .execute(&mut doc, &command)
                .unwrap();
            assert_eq!(
                &*image.objects[&source].geometry.geometry,
                doc.object(source).unwrap().geometry(),
                "{id}"
            );
            for (g, p) in &image.grips {
                assert_eq!(*p, doc.control_point_locations(g.object).unwrap()[g.index]);
            }
            let changed = preview.resolve(None, &doc, &display).0.unwrap();
            assert!(!Rc::ptr_eq(&image, &changed));
            if f.source_geometry(doc.tolerance())
                .unwrap()
                .curve_ref()
                .is_some()
            {
                doc.enable_edit_points([source]).unwrap();
                let mode_changed = preview.resolve(None, &doc, &display).0.unwrap();
                if f.mode() == GripMode::Control {
                    assert!(!Rc::ptr_eq(&changed, &mode_changed));
                }
            }
            let different = [ControlPointId {
                object: source,
                index: 1,
            }];
            let changed_picks = OrientSurfacePreview {
                grips: &different,
                ..preview
            };
            let image = changed_picks.resolve(None, &doc, &display).0.unwrap();
            assert_eq!(image.grips.len(), 1);
            assert!(image.grips.contains_key(&different[0]));
            let invalid = [ControlPointId {
                object: source,
                index: 999,
            }];
            assert!(
                OrientSurfacePreview {
                    grips: &invalid,
                    ..preview
                }
                .resolve(Some(q), &doc, &display)
                .0
                .is_none()
            );
        }
    }
    #[test]
    fn orient_surface_grip_markers_draw_at_deformed_positions_in_every_view() {
        use crate::viewport::preview_test_support::frame;
        let rows = recipes();
        let f: OrientSurfaceGripsFixture =
            serde_json::from_value(rows["operations"][2].clone()).unwrap();
        for kind in [
            ViewKind::Top,
            ViewKind::Front,
            ViewKind::Right,
            ViewKind::Perspective,
        ] {
            let mut doc = Document::default();
            let guide = doc
                .add_geometry(f.placement.guide(doc.tolerance()).unwrap())
                .unwrap();
            let source = doc
                .add_geometry(f.source_geometry(doc.tolerance()).unwrap())
                .unwrap();
            doc.enable_control_points([source]).unwrap();
            let picks = [ControlPointId {
                object: source,
                index: 0,
            }];
            doc.select_control_points(picks, SelectionMode::Add)
                .unwrap();
            let target =
                viboceros_command::orient_surface::Target::capture(&doc, guide, None).unwrap();
            let placement = Placement::try_new(
                Point3::try_from(f.placement.base).unwrap(),
                Point3::try_from(f.placement.reference).unwrap(),
                target,
                f.placement.options.options(),
                f.placement.plane(doc.tolerance()).unwrap(),
                doc.tolerance(),
            )
            .unwrap();
            let q = f.placement.target(f.placement.uv, doc.tolerance()).unwrap();
            let normal = Vector3::try_new(0., 0., 1.).unwrap();
            let cache = RefCell::new(OrientSurfacePreviewCache::default());
            let preview = OrientSurfacePreview {
                sources: &[],
                grips: &picks,
                placement: &placement,
                phase: PreviewPhase::Target,
                normal,
                last_normal: normal,
                last_point: Some(q),
                cache: &cache,
            };
            doc.clear_history().unwrap();
            let mut view = Viewport::new(kind);
            let context = egui::Context::default();
            let input = ViewportInput {
                orient_surface_preview: Some(preview),
                ..Default::default()
            };
            frame(&context, &mut view, &doc, input, vec![]);
            let posed = cache.borrow().image.as_ref().unwrap().grips[&picks[0]];
            let pixel = view.project(posed, view.last_rect.unwrap()).unwrap();
            let mut shapes = Vec::new();
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                        ..Default::default()
                    },
                    |ui| {
                        let _ = view.show(ui, &doc, input, &[], 0, true);
                    },
                )
                .shapes
                .into_iter()
                .for_each(|s| shapes.push(s.shape));
            fn marker(shape: &egui::Shape, pixel: Pos2) -> bool {
                match shape {
                    egui::Shape::Rect(r) => {
                        r.fill == SELECTED_COLOR
                            && r.rect.width() == 6.
                            && r.rect.center().distance(pixel) < 0.1
                    }
                    egui::Shape::Vec(s) => s.iter().any(|s| marker(s, pixel)),
                    _ => false,
                }
            }
            assert!(
                shapes.iter().any(|s| marker(s, pixel)),
                "{kind:?} missing destination grip marker"
            );
            assert!(doc.undo_label().is_none());
        }
    }
}
