//! Group-aware picking uses each seed's last ordered membership, not graph closure.
use super::*;

impl Document {
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
            return ids.filter(|id| self.is_object_selectable(*id)).collect();
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
