//! Parametric object/face hits retain a placed block leaf's identity.
use super::picking::PickHit;
use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// A leaf index addresses the immutable placed-member cache. Consumers retain
/// the root geometry snapshot and convert its member path to a persistent recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParametricPick {
    pub object: ObjectId,
    pub member: Option<usize>,
    pub face: Option<usize>,
}

impl Viewport {
    pub(super) fn pick_parametric_references(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        allowed: Option<&BTreeSet<ObjectId>>,
    ) -> Vec<ParametricPick> {
        if !pointer.is_finite() || !rect.contains(pointer) {
            return vec![];
        }
        let mut hits = vec![];
        for object in document.selectable_objects() {
            if allowed.is_some_and(|ids| !ids.contains(&object.id())) {
                continue;
            }
            let display = self
                .display_cache
                .borrow_mut()
                .get(object, document.tolerance());
            if let Geometry::BlockInstance(instance) = object.geometry() {
                let Ok(states) =
                    document.block_instance_member_displays(object.attributes(), instance)
                else {
                    continue;
                };
                for (member, (state, leaf)) in
                    states.iter().zip(display.block_members()).enumerate()
                {
                    if !state.visible || state.locked {
                        continue;
                    }
                    for (hit, face) in
                        self.parametric_display_hits(pointer, rect, leaf, document.tolerance())
                    {
                        hits.push((
                            hit,
                            ParametricPick {
                                object: object.id(),
                                member: Some(member),
                                face,
                            },
                        ));
                    }
                }
            } else {
                for (hit, face) in
                    self.parametric_display_hits(pointer, rect, &display, document.tolerance())
                {
                    hits.push((
                        hit,
                        ParametricPick {
                            object: object.id(),
                            member: None,
                            face,
                        },
                    ));
                }
            }
        }
        hits.retain(|(hit, _)| hit.distance.is_finite() && hit.distance <= PICK_CAPTURE_PIXELS);
        hits.sort_by(|(a, _), (b, _)| a.rank(*b));
        let Some((best, _)) = hits.first().copied() else {
            return vec![];
        };
        hits.into_iter()
            .filter(|(hit, _)| {
                if hit.is_curve_surface_overlap(best) {
                    true
                } else if self.display_mode == DisplayMode::Wireframe {
                    hit.is_ambiguous_without_depth(best)
                } else {
                    hit.is_ambiguous_with(best)
                }
            })
            .map(|(_, pick)| pick)
            .collect()
    }

    fn parametric_display_hits(
        &self,
        pointer: Pos2,
        rect: Rect,
        display: &display_cache::DisplayGeometry,
        tolerance: Tolerance,
    ) -> Vec<(PickHit, Option<usize>)> {
        if display.geometry.curve_ref().is_some() {
            return vec![(
                self.pick_display_geometry(pointer, rect, display, tolerance),
                None,
            )];
        }
        if !matches!(
            &*display.geometry,
            Geometry::Brep(_) | Geometry::NurbsSurface(_)
        ) {
            return vec![];
        }
        let Some(mesh) = display.mesh() else {
            return vec![];
        };
        let mut faces: BTreeMap<usize, PickHit> = BTreeMap::new();
        for (hit, triangle) in self.mesh_face_capture_hits(pointer, rect, mesh) {
            let face = match &*display.geometry {
                Geometry::Brep(_) => {
                    let Some(index) = display
                        .brep_face_sources()
                        .and_then(|indices| indices.get(triangle))
                    else {
                        continue;
                    };
                    *index
                }
                _ => 0,
            };
            faces
                .entry(face)
                .and_modify(|best| {
                    if hit.is_better_than(*best) {
                        *best = hit;
                    }
                })
                .or_insert(hit);
        }
        faces
            .into_iter()
            .map(|(face, hit)| (hit, Some(face)))
            .collect()
    }

    pub(super) fn paint_parametric_highlights(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        picks: &[ParametricPick],
    ) {
        for pick in picks {
            let Some(object) = document
                .object(pick.object)
                .filter(|_| document.is_object_selectable(pick.object))
            else {
                continue;
            };
            let root = self
                .display_cache
                .borrow_mut()
                .get(object, document.tolerance());
            let display = if let Some(index) = pick.member {
                let Geometry::BlockInstance(instance) = object.geometry() else {
                    continue;
                };
                let Some(member) = instance.members().get(index) else {
                    continue;
                };
                if !document
                    .block_member_display(object.attributes(), &member.path)
                    .is_ok_and(|state| state.visible && !state.locked)
                {
                    continue;
                }
                let Some(display) = root.block_members().get(index) else {
                    continue;
                };
                display
            } else {
                &root
            };
            let paint = |segments: &[[Point3; 2]]| {
                for &[a, b] in segments {
                    if let Some([a, b]) = self.project_selection_segment(a, b, rect) {
                        painter
                            .line_segment([a, b], Stroke::new(3., Color32::from_rgb(245, 160, 20)));
                    }
                }
            };
            if let Some(face) = pick.face {
                for edge in component_picking::face_edges(&display.geometry, face) {
                    if let Some(segments) = display.edges().get(edge) {
                        paint(segments);
                    }
                }
            } else {
                paint(display.wires());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::{BlockContent, BlockMember, BlockReference, ColorRgb};
    use viboceros_geometry::{AffineTransform3, Vector3};

    fn translation(x: Real, y: Real, z: Real) -> AffineTransform3 {
        AffineTransform3::try_new(
            [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            Vector3::try_new(x, y, z).unwrap(),
        )
        .unwrap()
    }
    fn setup() -> (Document, ObjectId, viboceros_document::LayerId) {
        let mut doc = Document::default();
        let registry = viboceros_command::CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Line 0,0,0 4,0,0").unwrap();
        let source = doc.objects().next().unwrap().id();
        let geometry = doc.object(source).unwrap().geometry_snapshot().clone();
        doc.delete_object(source).unwrap();
        let layer = doc.add_layer("Parts", ColorRgb::new(90, 110, 130)).unwrap();
        let leaf = doc
            .add_block_definition(
                "Leaf",
                vec![BlockMember::new(
                    BlockContent::Geometry(geometry),
                    ObjectAttributes::on_layer(layer),
                )],
            )
            .unwrap();
        let nested = doc
            .add_block_definition(
                "Pair",
                [0., 20.]
                    .map(|x| {
                        BlockMember::new(
                            BlockContent::Reference(
                                BlockReference::try_new(leaf, translation(x, 0., 0.)).unwrap(),
                            ),
                            ObjectAttributes::on_layer(doc.current_layer_id()),
                        )
                    })
                    .to_vec(),
            )
            .unwrap();
        let root = doc
            .add_block_instance(BlockReference::try_new(nested, translation(10., 5., 0.)).unwrap())
            .unwrap();
        doc.clear_history().unwrap();
        (doc, root, layer)
    }
    fn view(doc: &Document) -> (Viewport, Rect) {
        let mut view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(900., 700.));
        view.last_rect = Some(rect);
        view.zoom_extents(doc, Default::default()).unwrap();
        (view, rect)
    }
    #[test]
    fn nested_leaf_hits_keep_repeated_definition_paths_and_reuse_the_placed_source() {
        let (doc, root, _) = setup();
        let source = doc.object(root).unwrap().geometry_snapshot().clone();
        let (view, rect) = view(&doc);
        for (index, x) in [(0, 12.), (1, 32.)] {
            let point = Point3::try_new(x, 5., 0.).unwrap();
            let pixel = view.project(point, rect).unwrap();
            let picks = view.pick_parametric_references(pixel, rect, &doc, None);
            assert_eq!(
                picks,
                vec![ParametricPick {
                    object: root,
                    member: Some(index),
                    face: None
                }]
            );
            let Geometry::BlockInstance(instance) = doc.object(root).unwrap().geometry() else {
                panic!()
            };
            assert_eq!(
                instance.members()[index]
                    .path
                    .iter()
                    .map(|p| p.member_index)
                    .collect::<Vec<_>>(),
                vec![index, 0]
            );
        }
        assert!(
            doc.object(root)
                .unwrap()
                .geometry_snapshot()
                .shares_storage_with(&source)
        );
        assert!(!doc.can_undo());
    }
    #[test]
    fn coincident_curve_and_surface_remain_distinct_reference_choices() {
        let mut doc = Document::default();
        let registry = viboceros_command::CommandRegistry::with_builtins();
        registry
            .execute(&mut doc, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0")
            .unwrap();
        registry.execute(&mut doc, "Line 0,3,0 4,3,0").unwrap();
        let (mut view, rect) = view(&doc);
        let pixel = view
            .project(Point3::try_new(2., 3., 0.).unwrap(), rect)
            .unwrap();
        for mode in [DisplayMode::Wireframe, DisplayMode::Shaded] {
            view.display_mode = mode;
            let picks = view.pick_parametric_references(pixel, rect, &doc, None);
            assert_eq!(picks.len(), 2, "{picks:?}");
            assert!(picks[0].face.is_none());
            assert_eq!(picks[1].face, Some(0));
        }
    }

    #[test]
    fn wireframe_keeps_front_and_back_face_choices_while_shaded_picks_the_front() {
        for block in [false, true] {
            let mut doc = Document::default();
            viboceros_command::CommandRegistry::with_builtins()
                .execute(&mut doc, "Box 0,0,0 4,6,0 2")
                .unwrap();
            let mut id = doc.objects().next().unwrap().id();
            if block {
                let source = doc.object(id).unwrap().geometry_snapshot().clone();
                doc.delete_object(id).unwrap();
                let definition = doc
                    .add_block_definition(
                        "Box",
                        vec![BlockMember::new(
                            BlockContent::Geometry(source),
                            ObjectAttributes::on_layer(doc.current_layer_id()),
                        )],
                    )
                    .unwrap();
                id = doc
                    .add_block_instance(
                        BlockReference::try_new(definition, translation(0., 0., 0.)).unwrap(),
                    )
                    .unwrap();
            }
            let (mut view, rect) = view(&doc);
            let pixel = view
                .project(Point3::try_new(2., 3., 2.).unwrap(), rect)
                .unwrap();
            view.display_mode = DisplayMode::Wireframe;
            let choices = view.pick_parametric_references(pixel, rect, &doc, None);
            assert_eq!(choices.len(), 2, "{block}: {choices:?}");
            assert!(
                choices
                    .iter()
                    .all(|p| p.object == id && p.member == block.then_some(0))
            );
            assert_ne!(choices[0].face, choices[1].face);
            view.display_mode = DisplayMode::Shaded;
            assert_eq!(
                view.pick_parametric_references(pixel, rect, &doc, None),
                choices[..1]
            );
        }
    }

    #[test]
    fn prototype_visibility_locking_and_object_restrictions_filter_reference_hits() {
        let (mut doc, root, layer) = setup();
        let (view, rect) = view(&doc);
        let pixel = view
            .project(Point3::try_new(12., 5., 0.).unwrap(), rect)
            .unwrap();
        assert!(
            !view
                .pick_parametric_references(pixel, rect, &doc, None)
                .is_empty()
        );
        assert!(
            view.pick_parametric_references(pixel, rect, &doc, Some(&BTreeSet::new()))
                .is_empty()
        );
        doc.set_layer_visibility(layer, false).unwrap();
        assert!(
            view.pick_parametric_references(pixel, rect, &doc, None)
                .is_empty()
        );
        doc.set_layer_visibility(layer, true).unwrap();
        doc.set_layer_locked(layer, true).unwrap();
        // Definition layer locking does not lock a placed part. The owning
        // instance's layer still controls whether the reference can be picked.
        assert_eq!(
            view.pick_parametric_references(pixel, rect, &doc, None)
                .len(),
            1
        );
        let root_layer = doc.object(root).unwrap().attributes().layer_id();
        let current = doc.add_layer("Current", ColorRgb::new(0, 0, 0)).unwrap();
        doc.set_current_layer(current).unwrap();
        doc.set_layer_locked(root_layer, true).unwrap();
        assert!(
            view.pick_parametric_references(pixel, rect, &doc, None)
                .is_empty()
        );
        doc.set_layer_locked(root_layer, false).unwrap();
        assert_eq!(
            view.pick_parametric_references(pixel, rect, &doc, Some(&BTreeSet::from([root])))
                .len(),
            1
        );
    }
}
