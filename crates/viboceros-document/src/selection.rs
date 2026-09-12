//! Group-aware picking uses each seed's last ordered membership, not graph closure.
use super::*;
mod objects;
pub(super) fn selected_objects(document: &Document) -> impl Iterator<Item = &Object> {
    objects::SelectedObjects::new(document)
}

impl Document {
    /// Replaces selection with exact outputs of a document-editing command.
    /// Unlike picking, this neither expands groups nor rejects hidden/locked
    /// results inherited from a selected source. UI picking must use
    /// `select_objects` or `select_objects_direct` instead.
    /// Missing IDs are rejected before any selection/history state changes.
    pub fn select_command_results(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        let indices = self.resolve_object_indices(ids)?;
        let selected = indices
            .into_iter()
            .map(|index| self.objects[index].id)
            .collect();
        Ok(self.update_selection(selected))
    }

    /// Attribute/layer changes prune individual objects, without group expansion.
    /// History replay has a separate group-aware cleanup policy below.
    pub(super) fn prune_selection(&mut self) {
        let selection = self.selectable_recorded_objects(&self.selection);
        self.update_selection(selection);
    }

    /// Validate the complete request before expanding groups or mutating selection.
    /// Missing IDs take precedence over unselectable seeds, in sorted-ID order.
    pub(super) fn validate_selection_seeds(
        &self,
        ids: &BTreeSet<ObjectId>,
    ) -> Result<(), DocumentError> {
        if ids.len() <= 16 {
            if let Some(id) = ids.iter().find(|id| self.object(**id).is_none()) {
                return Err(DocumentError::ObjectNotFound(*id));
            }
            if let Some(id) = ids.iter().find(|id| !self.is_object_selectable(**id)) {
                return Err(DocumentError::ObjectNotSelectable(*id));
            }
            return Ok(());
        }
        let layers = self
            .layers
            .iter()
            .filter(|l| l.visible && !l.locked)
            .map(|l| l.id)
            .collect::<BTreeSet<_>>();
        let mut missing = ids.clone();
        let mut unselectable: Option<ObjectId> = None;
        for object in &self.objects {
            if !missing.remove(&object.id) {
                continue;
            }
            let attributes = &object.attributes;
            if !attributes.visible || attributes.locked || !layers.contains(&attributes.layer_id) {
                unselectable = Some(unselectable.map_or(object.id, |id| id.min(object.id)));
            }
        }
        if let Some(id) = missing.first() {
            return Err(DocumentError::ObjectNotFound(*id));
        }
        if let Some(id) = unselectable {
            return Err(DocumentError::ObjectNotSelectable(id));
        }
        Ok(())
    }

    pub(super) fn previous_selection_targets(&self) -> BTreeSet<ObjectId> {
        self.selectable_recorded_objects(&self.previous_selection)
    }

    pub(super) fn selectable_recorded_objects(
        &self,
        recorded: &BTreeSet<ObjectId>,
    ) -> BTreeSet<ObjectId> {
        if recorded.is_empty() {
            return BTreeSet::new();
        }
        let layers = self
            .layers
            .iter()
            .filter(|layer| layer.visible && !layer.locked)
            .map(|layer| layer.id)
            .collect::<BTreeSet<_>>();
        self.objects
            .iter()
            .filter(|object| {
                recorded.contains(&object.id)
                    && object.attributes.visible
                    && !object.attributes.locked
                    && layers.contains(&object.attributes.layer_id)
            })
            .map(|object| object.id)
            .collect()
    }

    /// Hidden/locked members can enter the selection through a selectable
    /// group peer. Rhino allows editing that selected set without unlocking it.
    pub(super) fn ensure_object_editable(&self, object: &Object) -> Result<(), DocumentError> {
        if object.attributes.locked && !self.is_selected(object.id) {
            return Err(DocumentError::ObjectLocked(object.id));
        }
        let layer = self
            .layer(object.attributes.layer_id)
            .ok_or(DocumentError::LayerNotFound(object.attributes.layer_id))?;
        if layer.locked && !self.is_selected(object.id) {
            return Err(DocumentError::LayerLocked(layer.id));
        }
        Ok(())
    }

    pub(super) fn prune_selection_after_history(&mut self) {
        let mut seen = BTreeSet::new();
        self.selection_order
            .retain(|id| self.selection.contains(id) && seen.insert(*id));
        let allowed = self.selectable_clusters(self.selection.iter().copied());
        let next = self.selection.intersection(&allowed).copied().collect();
        self.update_selection(next);
    }

    pub(super) fn selectable_clusters(
        &self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> BTreeSet<ObjectId> {
        let mut ids = ids.into_iter().peekable();
        if ids.peek().is_none() {
            return BTreeSet::new();
        }
        if self.groups.is_empty() {
            // Batch history can restore thousands of selected objects. Scan
            // the object table once instead of resolving each ID linearly.
            return self.selectable_recorded_objects(&ids.collect());
        }
        let layers = self
            .layers
            .iter()
            .filter(|layer| layer.visible && !layer.locked)
            .map(|layer| layer.id)
            .collect::<BTreeSet<_>>();
        let objects = self
            .objects
            .iter()
            .map(|object| (object.id, object))
            .collect::<BTreeMap<_, _>>();
        let selectable = |object: &Object| {
            let attributes = object.attributes();
            attributes.visible && !attributes.locked && layers.contains(&attributes.layer_id)
        };
        let mut targets = BTreeSet::new();
        let mut groups = BTreeSet::new();
        for id in ids {
            if let Some(object) = objects.get(&id).filter(|object| selectable(object)) {
                targets.insert(id);
                if let Some(group) = object.top_group() {
                    groups.insert(group);
                }
            }
        }
        // Expand only original seeds' groups, never reached peers' memberships.
        for group in self
            .groups
            .iter()
            .filter(|group| groups.contains(&group.id))
        {
            targets.extend(
                group
                    .members
                    .iter()
                    .copied()
                    .filter(|id| objects.contains_key(id)),
            );
        }
        targets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_results_are_exact_allow_restricted_outputs_and_validate_before_mutation() {
        let mut document = Document::default();
        let ids = [0., 1., 2.].map(|x| {
            document
                .add_geometry(Geometry::Point(Point3::try_new(x, 0., 0.).unwrap()))
                .unwrap()
        });
        document.add_group(None, [ids[0], ids[1]]).unwrap();
        document.add_group(None, [ids[1], ids[2]]).unwrap();
        document.set_objects_locked([ids[1]], true).unwrap();
        document.set_objects_visibility([ids[2]], false).unwrap();
        document
            .select_command_results([ids[2], ids[1], ids[1]])
            .unwrap();
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([ids[1], ids[2]])
        );
        let missing = ObjectId::new();
        let before = format!("{document:?}");
        assert!(
            matches!(document.select_command_results([ids[0], missing]), Err(DocumentError::ObjectNotFound(id)) if id == missing)
        );
        assert_eq!(format!("{document:?}"), before);
        assert!(matches!(
            document.select_objects_direct([ids[1]], SelectionMode::Replace),
            Err(DocumentError::ObjectNotSelectable(_))
        ));
        assert_eq!(format!("{document:?}"), before);
        document.select_command_results([]).unwrap();
        assert_eq!(document.selected_object_count(), 0);
    }

    #[test]
    fn selectable_iteration_obeys_object_and_layer_modes_without_group_expansion() {
        let mut document = Document::default();
        let mut expected = Vec::new();
        let mut ids = Vec::new();
        for layer_mode in 0..4 {
            let layer = document
                .add_layer(format!("Layer-{layer_mode}"), ColorRgb::BLACK)
                .unwrap();
            for object_mode in 0..4 {
                let id = document
                    .add_geometry(Geometry::Point(
                        Point3::try_new(layer_mode as f64, object_mode as f64, 0.).unwrap(),
                    ))
                    .unwrap();
                let object = document.objects.last_mut().unwrap();
                object.attributes.layer_id = layer;
                object.attributes.visible = object_mode & 1 == 0;
                object.attributes.locked = object_mode & 2 != 0;
                if layer_mode == 0 && object_mode == 0 {
                    expected.push(id);
                }
                ids.push(id);
            }
            let record = document
                .layers
                .iter_mut()
                .find(|record| record.id == layer)
                .unwrap();
            record.visible = layer_mode & 1 == 0;
            record.locked = layer_mode & 2 != 0;
        }
        document.add_group(Some("All".into()), ids).unwrap();
        let before = format!("{document:?}");
        assert_eq!(
            document
                .selectable_objects()
                .map(Object::id)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(format!("{document:?}"), before);
        assert_eq!(document.select_group_objects_by_name("All"), expected.len());
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), expected);
    }

    #[test]
    fn batch_pruning_matches_per_object_policy_and_preserves_other_state() {
        let mut fixture = Document::default();
        let ids = points(&mut fixture, 64);
        let hidden = fixture.add_layer("hidden", ColorRgb::BLACK).unwrap();
        let locked = fixture.add_layer("locked", ColorRgb::BLACK).unwrap();
        fixture.set_layer_visibility(hidden, false).unwrap();
        fixture.set_layer_locked(locked, true).unwrap();
        fixture.add_group(None, ids.iter().copied()).unwrap();
        // Seed redo and selection memories independently of the tested cleanup.
        fixture
            .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
            .unwrap();
        fixture.undo().unwrap();
        for count in [0, 1, 16, 17, 32, 64] {
            for offset in [0, 7, 19] {
                let mut document = fixture.clone();
                for index in (0..count).rev() {
                    let id = ids[(offset + index) % ids.len()];
                    document
                        .select_objects_direct([id], SelectionMode::Add)
                        .unwrap();
                }
                document.previous_selection = ids[..3].iter().copied().collect();
                document.previous_selection_order = ids[..3].to_vec();
                for (i, object) in document.objects.iter_mut().enumerate() {
                    match i % 5 {
                        0 => object.attributes.visible = false,
                        1 => object.attributes.locked = true,
                        2 => object.attributes.layer_id = hidden,
                        3 => object.attributes.layer_id = locked,
                        _ => {}
                    }
                }
                let mut reference = document.clone();
                let expected = reference
                    .selection
                    .iter()
                    .copied()
                    .filter(|id| reference.is_object_selectable(*id))
                    .collect();
                reference.update_selection(expected);
                document.prune_selection();
                assert_eq!(
                    format!("{document:?}"),
                    format!("{reference:?}"),
                    "count={count}, offset={offset}"
                );
                let once = format!("{document:?}");
                document.prune_selection();
                assert_eq!(format!("{document:?}"), once, "cleanup is idempotent");
            }
        }
    }

    #[test]
    fn layer_pruning_keeps_pick_order_and_rolls_back_selection_memories() {
        for lock in [false, true] {
            let mut document = Document::default();
            let layer = document.add_layer("target", ColorRgb::BLACK).unwrap();
            let ids = points(&mut document, 40);
            for object in document.objects.iter_mut().step_by(2) {
                object.attributes.layer_id = layer;
            }
            for id in ids.iter().rev() {
                document
                    .select_objects_direct([*id], SelectionMode::Add)
                    .unwrap();
            }
            let before = document.clone();
            document.begin_transaction("layer change").unwrap();
            if lock {
                document.set_layer_locked(layer, true).unwrap();
            } else {
                document.set_layer_visibility(layer, false).unwrap();
            }
            assert_eq!(
                document.selected_object_ids().collect::<Vec<_>>(),
                ids.iter()
                    .skip(1)
                    .step_by(2)
                    .rev()
                    .copied()
                    .collect::<Vec<_>>()
            );
            assert_eq!(document.previous_selection_order, before.selection_order);
            document.rollback_transaction().unwrap();
            assert_eq!(document.layers, before.layers);
            assert_eq!(document.objects, before.objects);
            assert_eq!(document.selection_order, before.selection_order);
            assert_eq!(document.selection, before.selection);
            assert_eq!(document.previous_selection, before.previous_selection);
            assert_eq!(
                document.previous_selection_order,
                before.previous_selection_order
            );
        }
    }

    #[test]
    fn table_filters_preserve_visibility_color_group_and_name_policy() {
        let mut document = Document::default();
        let hidden = document.add_layer("hidden", ColorRgb::BLACK).unwrap();
        let locked = document.add_layer("locked", ColorRgb::BLACK).unwrap();
        document.set_layer_visibility(hidden, false).unwrap();
        document.set_layer_locked(locked, true).unwrap();
        let red = ColorRgb::new(255, 0, 0);
        let blue = ColorRgb::new(0, 0, 255);
        let ids = (0..8)
            .map(|i| {
                document
                    .add_geometry_with_attributes(
                        Geometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap()),
                        ObjectAttributes::on_layer(document.current_layer_id())
                            .with_name(if i == 5 || i == 7 { "miss" } else { "hit" })
                            .with_object_color(if i == 5 { blue } else { red }),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        document.objects[1].attributes.visible = false;
        document.objects[2].attributes.locked = true;
        document.objects[3].attributes.layer_id = hidden;
        document.objects[4].attributes.layer_id = locked;
        document.add_group(None, [ids[6], ids[7]]).unwrap();
        let objects = document.objects.clone();
        let layers = document.layers.clone();
        let groups = document.groups.clone();
        let undo = document.undo_label().map(str::to_owned);
        assert_eq!(document.select_all(), 4);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            [ids[0], ids[5], ids[6], ids[7]]
        );
        document
            .select_objects_direct([ids[0]], SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.invert_selection(), 3);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            [ids[5], ids[6], ids[7]]
        );
        document.clear_selection();
        assert_eq!(document.select_objects_by_name_pattern("H?T"), 2);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            [ids[0], ids[6]]
        );
        document.clear_selection();
        assert_eq!(document.select_objects_by_display_color(red).unwrap(), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[0]]);
        assert_eq!(document.objects, objects);
        assert_eq!(document.layers, layers);
        assert_eq!(document.groups, groups);
        assert_eq!(document.undo_label(), undo.as_deref());
    }

    #[test]
    fn batched_seed_validation_matches_independent_per_id_checks() {
        let mut document = Document::default();
        let ids = points(&mut document, 64);
        let layer = document.add_layer("locked", ColorRgb::BLACK).unwrap();
        document.set_layer_locked(layer, true).unwrap();
        // Test both object flags and layer policy, including a non-current layer.
        document.objects[3].attributes.visible = false;
        document.objects[20].attributes.locked = true;
        document.objects[45].attributes.layer_id = layer;
        for count in [0, 1, 16, 17, 32, 64] {
            for offset in [0, 7, 19] {
                let mut requested = (0..count)
                    .map(|i| ids[(i + offset) % ids.len()])
                    .collect::<BTreeSet<_>>();
                for missing in [false, true] {
                    if missing {
                        requested.insert(ObjectId::new());
                    }
                    let expected = if let Some(id) =
                        requested.iter().find(|id| document.object(**id).is_none())
                    {
                        Err(DocumentError::ObjectNotFound(*id))
                    } else if let Some(id) = requested
                        .iter()
                        .find(|id| !document.is_object_selectable(**id))
                    {
                        Err(DocumentError::ObjectNotSelectable(*id))
                    } else {
                        Ok(())
                    };
                    assert_eq!(
                        document.validate_selection_seeds(&requested),
                        expected,
                        "count={count} offset={offset}"
                    );
                }
            }
        }
    }

    #[test]
    fn seed_validation_rejects_each_object_and_layer_visibility_state() {
        for count in [1, 17] {
            for blocked in 0..4 {
                let mut document = Document::default();
                let ids = points(&mut document, count);
                let target = ids[count - 1];
                match blocked {
                    0 => document.objects[count - 1].attributes.visible = false,
                    1 => document.objects[count - 1].attributes.locked = true,
                    _ => {
                        let layer = document.add_layer("blocked", ColorRgb::BLACK).unwrap();
                        document.objects[count - 1].attributes.layer_id = layer;
                        if blocked == 2 {
                            document.set_layer_visibility(layer, false).unwrap();
                        } else {
                            document.set_layer_locked(layer, true).unwrap();
                        }
                    }
                }
                assert_eq!(
                    document.validate_selection_seeds(&ids.into_iter().collect()),
                    Err(DocumentError::ObjectNotSelectable(target))
                );
            }
        }
    }

    #[test]
    fn invalid_large_selection_is_atomic_for_all_modes_and_both_selection_paths() {
        for direct in [false, true] {
            for mode in [
                SelectionMode::Replace,
                SelectionMode::Add,
                SelectionMode::Remove,
                SelectionMode::Toggle,
            ] {
                let mut document = Document::default();
                let ids = points(&mut document, 32);
                document
                    .select_object(ids[3], SelectionMode::Replace)
                    .unwrap();
                document
                    .set_objects_visibility([ids[1], ids[17]], false)
                    .unwrap();
                document
                    .add_geometry(Geometry::Point(Point3::try_new(99., 0., 0.).unwrap()))
                    .unwrap();
                document.undo().unwrap();
                let before = format!("{document:?}");
                for missing in [false, true] {
                    let mut requested = ids.clone();
                    let absent = ObjectId::new();
                    if missing {
                        requested.push(absent);
                    }
                    let result = if direct {
                        document.select_objects_direct(requested, mode)
                    } else {
                        document.select_objects(requested, mode)
                    };
                    let expected = if missing {
                        DocumentError::ObjectNotFound(absent)
                    } else {
                        DocumentError::ObjectNotSelectable(ids[1].min(ids[17]))
                    };
                    assert_eq!(result, Err(expected));
                    assert_eq!(format!("{document:?}"), before);
                }
            }
        }
    }

    #[test]
    fn large_valid_seed_sets_expand_only_their_groups_and_keep_batch_order() {
        let mut document = Document::default();
        let ids = points(&mut document, 40);
        document.add_group(None, [ids[0], ids[30]]).unwrap();
        document.add_group(None, [ids[30], ids[31]]).unwrap();
        document.set_objects_visibility([ids[30]], false).unwrap();
        let seeds = ids[..20]
            .iter()
            .rev()
            .copied()
            .chain(ids[..20].iter().copied())
            .collect::<Vec<_>>();
        document
            .select_objects_direct(seeds.clone(), SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            ids[..20]
        );
        document
            .select_objects(seeds, SelectionMode::Replace)
            .unwrap();
        let expected = ids[..20]
            .iter()
            .copied()
            .chain([ids[30]])
            .collect::<Vec<_>>();
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), expected);
        assert!(!document.is_selected(ids[31]));
    }

    #[test]
    #[ignore = "manual large selection iteration timing"]
    fn benchmark_ordered_selection_iteration() {
        let mut document = Document::default();
        document.begin_transaction("fixture").unwrap();
        let ids = (0..20_000)
            .map(|i| {
                document
                    .add_geometry(Geometry::Point(
                        Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
                    ))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        document.commit_transaction().unwrap();
        let selection_start = std::time::Instant::now();
        document
            .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let selection_elapsed = selection_start.elapsed();
        let start = std::time::Instant::now();
        let actual = document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let elapsed = start.elapsed();
        assert_eq!(actual, ids);
        eprintln!("20k selected objects, ordered iteration: {elapsed:?}");
        eprintln!("20k objects, direct selection: {selection_elapsed:?}");
        document.clear_selection();
        let start = std::time::Instant::now();
        assert_eq!(document.select_all(), ids.len());
        eprintln!("20k objects, select all: {:?}", start.elapsed());
        document.clear_selection();
        let start = std::time::Instant::now();
        assert_eq!(document.select_objects_by_name_pattern("*"), ids.len());
        eprintln!("20k objects, wildcard selection: {:?}", start.elapsed());
        let layer = document
            .add_layer("prune benchmark", ColorRgb::BLACK)
            .unwrap();
        for object in document.objects.iter_mut().step_by(2) {
            object.attributes.layer_id = layer;
        }
        let start = std::time::Instant::now();
        document.set_layer_visibility(layer, false).unwrap();
        eprintln!(
            "20k selected objects, hide half by layer: {:?}",
            start.elapsed()
        );
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            ids.iter().skip(1).step_by(2).copied().collect::<Vec<_>>()
        );
    }

    fn points(document: &mut Document, count: usize) -> Vec<ObjectId> {
        (0..count)
            .map(|i| {
                document
                    .add_geometry(Geometry::Point(
                        Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
                    ))
                    .unwrap()
            })
            .collect()
    }

    #[test]
    fn repeated_object_edits_replay_without_duplicate_selection_order() {
        let mut document = Document::default();
        let id = points(&mut document, 1)[0];
        document.begin_transaction("Repeated edits").unwrap();
        for selected in [true, false, true] {
            document.clear_selection();
            if selected {
                document.select_object(id, SelectionMode::Add).unwrap();
            }
            document
                .transform_objects(
                    [id],
                    AffineTransform3::from_translation(
                        viboceros_geometry::Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
                    ),
                )
                .unwrap();
        }
        document.commit_transaction().unwrap();
        for _ in 0..3 {
            document.undo().unwrap();
            assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [id]);
            document.redo().unwrap();
            assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [id]);
        }
    }

    #[test]
    fn deletion_and_its_replay_preserve_last_changed_members() {
        for removed in [vec![0], vec![2], vec![0, 1], vec![0, 1, 2]] {
            let mut document = Document::default();
            let ids = points(&mut document, 3);
            document.add_group(None, [ids[0], ids[1]]).unwrap();
            document.begin_transaction("Move pair").unwrap();
            document
                .transform_objects(
                    [ids[0], ids[1]],
                    AffineTransform3::from_translation(
                        viboceros_geometry::Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
                    ),
                )
                .unwrap();
            document.commit_transaction().unwrap();
            document.begin_transaction("Delete subset").unwrap();
            for &index in &removed {
                document.delete_object(ids[index]).unwrap();
            }
            document.commit_transaction().unwrap();
            let survivors = [0, 1]
                .into_iter()
                .filter(|i| !removed.contains(i))
                .map(|i| ids[i])
                .collect::<BTreeSet<_>>();
            for replay in 0..4 {
                document.select_last_changed(true);
                assert_eq!(
                    document.selected_object_ids().collect::<BTreeSet<_>>(),
                    if replay % 2 == 0 {
                        survivors.clone()
                    } else {
                        BTreeSet::from([ids[0], ids[1]])
                    }
                );
                if replay % 2 == 0 {
                    document.undo().unwrap();
                } else {
                    document.redo().unwrap();
                }
            }
        }
    }

    #[test]
    fn adding_an_empty_layer_preserves_last_changed_objects() {
        let mut document = Document::default();
        let ids = points(&mut document, 3);
        document.add_layer("Empty", ColorRgb::new(0, 0, 0)).unwrap();
        assert_eq!(document.select_last_changed(true), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[2]]);
    }

    #[test]
    fn last_selection_recalls_changed_selectable_objects_not_their_groups() {
        for mode in 0..5 {
            let mut document = Document::default();
            let ids = points(&mut document, 3);
            document.add_group(None, [ids[0], ids[1]]).unwrap();
            document.add_group(None, [ids[1], ids[2]]).unwrap();
            match mode {
                1 => {
                    document.set_objects_locked([ids[1]], true).unwrap();
                }
                2 => {
                    document.set_objects_visibility([ids[1]], false).unwrap();
                }
                3 | 4 => {
                    let layer = document
                        .add_layer("Bridge", ColorRgb::new(0, 0, 0))
                        .unwrap();
                    document.set_objects_layer([ids[1]], layer).unwrap();
                    if mode == 3 {
                        document.set_layer_locked(layer, true).unwrap();
                    } else {
                        document.set_layer_visibility(layer, false).unwrap();
                    }
                }
                _ => {}
            }
            document
                .select_object(ids[0], SelectionMode::Replace)
                .unwrap();
            document
                .transform_objects(
                    [ids[0], ids[1]],
                    AffineTransform3::from_translation(
                        viboceros_geometry::Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
                    ),
                )
                .unwrap();
            let expected = if mode == 0 {
                vec![ids[0], ids[1]]
            } else {
                vec![ids[0]]
            };
            assert_eq!(
                document.selectable_last_changed_object_count(),
                expected.len()
            );
            document.select_last_changed(true);
            assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), expected);
            assert!(!document.is_selected(ids[2]));
            document
                .select_objects_direct([ids[2]], SelectionMode::Replace)
                .unwrap();
            assert_eq!(document.select_last_changed(false), expected.len() + 1);
        }
    }

    #[test]
    fn recall_keeps_exact_recorded_ids_order_and_nonempty_memory() {
        let mut document = Document::default();
        let ids = points(&mut document, 3);
        document.add_group(None, [ids[0], ids[1]]).unwrap();
        let second = document.add_group(None, [ids[1], ids[2]]).unwrap();
        document
            .select_objects_direct([ids[0]], SelectionMode::Replace)
            .unwrap();
        document.clear_selection();
        document.add_group_members(second, [ids[0]]).unwrap();
        assert_eq!(document.selectable_previous_object_count(), 1);
        for _ in 0..3 {
            assert_eq!(document.select_previous(true), 1);
            assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[0]]);
        }
        document.clear_selection();
        document
            .select_objects_direct([ids[2]], SelectionMode::Add)
            .unwrap();
        document
            .select_objects_direct([ids[1]], SelectionMode::Add)
            .unwrap();
        for _ in 0..3 {
            assert_eq!(document.select_previous(false), 3);
            assert_eq!(
                document.selected_object_ids().collect::<Vec<_>>(),
                [ids[2], ids[1], ids[0]]
            );
            assert_eq!(document.selectable_previous_object_count(), 1);
        }
        document.select_previous(true);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[0]]);
        document.select_previous(true);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            [ids[2], ids[1], ids[0]]
        );
    }

    #[test]
    fn overlapping_group_picks_match_rhino_move_mouse_probes() {
        let mut document = Document::default();
        let ids = points(&mut document, 3);
        let first = document.add_group(None, [ids[0], ids[1]]).unwrap();
        let second = document.add_group(None, [ids[1], ids[2]]).unwrap();
        for (reverse, expected) in [
            (false, [[0, 1], [1, 2], [1, 2]]),
            (true, [[0, 1], [0, 1], [1, 2]]),
        ] {
            document
                .set_object_group_memberships(
                    ids[1],
                    if reverse {
                        [second, first]
                    } else {
                        [first, second]
                    },
                )
                .unwrap();
            for (seed, expected) in expected.into_iter().enumerate() {
                document.clear_selection();
                document
                    .select_object(ids[seed], SelectionMode::Replace)
                    .unwrap();
                assert_eq!(
                    document.selected_object_ids().collect::<Vec<_>>(),
                    expected.map(|i| ids[i])
                );
                document.groups.reverse();
                assert_eq!(
                    document.selectable_clusters([ids[seed]]),
                    BTreeSet::from(expected.map(|i| ids[i]))
                );
            }
        }
        document
            .select_objects([ids[0], ids[2]], SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.selection, ids.iter().copied().collect());
        document
            .select_object(ids[0], SelectionMode::Remove)
            .unwrap();
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[2]]);
    }

    // Simple seed-by-seed reference, deliberately without shared indices.
    fn reference(document: &Document, seeds: &[ObjectId]) -> BTreeSet<ObjectId> {
        let mut targets = BTreeSet::new();
        for &seed in seeds {
            if !document.is_object_selectable(seed) {
                continue;
            }
            targets.insert(seed);
            if let Some(group) = document.object(seed).unwrap().top_group() {
                targets.extend(
                    document
                        .group(group)
                        .unwrap()
                        .members
                        .iter()
                        .copied()
                        .filter(|id| document.object(*id).is_some()),
                );
            }
        }
        targets
    }

    #[test]
    fn exhaustive_three_object_group_tables_match_seed_local_policy() {
        let mut document = Document::default();
        let ids = points(&mut document, 3);
        let missing = ObjectId::new();
        for graph in 0..128 {
            document.groups.clear();
            for members in 1..8 {
                if graph & (1 << (members - 1)) != 0 {
                    document.groups.push(Group {
                        id: GroupId::new(),
                        name: None,
                        members: ids
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| members & (1 << i) != 0)
                            .map(|(_, id)| *id)
                            .collect(),
                    });
                }
            }
            for order in 0..8 {
                for (i, object) in document.objects.iter_mut().enumerate() {
                    object.group_ids = document
                        .groups
                        .iter()
                        .filter(|group| group.members.contains(&object.id))
                        .map(|group| group.id)
                        .collect();
                    if order & (1 << i) != 0 {
                        object.group_ids.reverse();
                    }
                }
                for eligibility in 0..8 {
                    for (i, object) in document.objects.iter_mut().enumerate() {
                        object.attributes.locked = eligibility & (1 << i) == 0;
                    }
                    for seeds in 0..8 {
                        let mut seeds = ids
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| seeds & (1 << i) != 0)
                            .map(|(_, id)| *id)
                            .collect::<Vec<_>>();
                        let expected = reference(&document, &seeds);
                        seeds.extend(seeds.clone());
                        seeds.push(missing);
                        assert_eq!(document.selectable_clusters(seeds.clone()), expected);
                        document.groups.reverse();
                        assert_eq!(document.selectable_clusters(seeds), expected);
                    }
                }
            }
        }
    }

    #[test]
    fn long_chain_does_not_propagate_beyond_the_seed_group() {
        let mut document = Document::default();
        let ids = points(&mut document, 1024);
        for pair in ids.windows(2).rev() {
            document.add_group(None, pair.iter().copied()).unwrap();
        }
        assert_eq!(
            document.selectable_clusters([ids[0]]),
            BTreeSet::from([ids[0], ids[1]])
        );
        assert_eq!(
            document.selectable_clusters(ids.iter().copied()),
            ids.into_iter().collect()
        );
    }

    #[test]
    fn grouped_hidden_and_locked_members_are_selected_and_editable_without_unlocking() {
        for locked in [false, true] {
            let mut document = Document::default();
            let ids = points(&mut document, 3);
            document.add_group(None, [ids[0], ids[1]]).unwrap();
            document.add_group(None, [ids[1], ids[2]]).unwrap();
            if locked {
                document.set_objects_locked([ids[1]], true).unwrap();
            } else {
                document.set_objects_visibility([ids[1]], false).unwrap();
            }
            assert_eq!(
                document.select_object(ids[1], SelectionMode::Replace),
                Err(DocumentError::ObjectNotSelectable(ids[1]))
            );
            let transform = AffineTransform3::from_translation(
                viboceros_geometry::Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            );
            if locked {
                assert_eq!(
                    document.transform_objects([ids[1]], transform),
                    Err(DocumentError::ObjectLocked(ids[1]))
                );
            }
            document
                .select_object(ids[0], SelectionMode::Replace)
                .unwrap();
            assert_eq!(document.selection, BTreeSet::from([ids[0], ids[1]]));
            let before = document.objects.clone();
            document
                .transform_objects([ids[0], ids[1]], transform)
                .unwrap();
            assert_eq!(document.objects[1].attributes, before[1].attributes);
            assert_eq!(document.objects[2], before[2]);
            document.undo().unwrap();
            assert_eq!(document.objects, before);
            assert_eq!(document.selection, BTreeSet::from([ids[0], ids[1]]));
            document.redo().unwrap();
            assert_eq!(document.selection, BTreeSet::from([ids[0], ids[1]]));
            document.begin_transaction("rollback grouped edit").unwrap();
            document
                .transform_objects([ids[0], ids[1]], transform)
                .unwrap();
            document.rollback_transaction().unwrap();
            assert_eq!(document.selection, BTreeSet::from([ids[0], ids[1]]));
            document.clear_selection();
            if locked {
                assert_eq!(
                    document.transform_objects([ids[1]], transform),
                    Err(DocumentError::ObjectLocked(ids[1]))
                );
            }
            document
                .select_object(ids[0], SelectionMode::Replace)
                .unwrap();
            let destination = document
                .add_layer("Destination", ColorRgb::new(1, 2, 3))
                .unwrap();
            document
                .set_objects_layer([ids[0], ids[1]], destination)
                .unwrap();
            assert_eq!(
                document.object(ids[1]).unwrap().attributes.layer_id,
                destination
            );
            assert_eq!(document.object(ids[1]).unwrap().attributes.locked, locked);
        }
    }
}
