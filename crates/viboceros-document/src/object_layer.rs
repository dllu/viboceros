use super::history::Edit;
use super::{Document, DocumentError, LayerId, Object, ObjectId, ObjectIsolation};

impl Document {
    /// Atomically moves the requested editable objects to an existing layer.
    ///
    /// Geometry, object identity, object-level modes, group membership, and
    /// the current layer are retained. Hidden and locked destinations are
    /// valid; objects moved there are pruned from the transient selection.
    pub fn set_objects_layer(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        layer_id: LayerId,
    ) -> Result<usize, DocumentError> {
        self.layer_index(layer_id)?;
        let staged = self
            .editable_layer_object_indices(ids)?
            .into_iter()
            .filter(|index| self.objects[*index].attributes.layer_id != layer_id)
            .collect::<Vec<_>>();
        if staged.is_empty() {
            return Ok(0);
        }

        let changed_count = staged.len();
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Set object layer")?;
        }
        for index in staged {
            let before = super::ObjectProperties::from(&self.objects[index]);
            let mut after = before.clone();
            after.attributes.layer_id = layer_id;
            let id = after.id;
            after.apply_to(&mut self.objects[index]);
            self.record_edit(
                "Set object layer",
                Edit::ObjectPropertiesChanged {
                    id,
                    selected: self.is_selected(id),
                    states: Box::new([before, after]),
                },
            );
        }
        self.prune_selection();
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(changed_count)
    }

    /// Copies the requested editable objects in place onto an existing layer.
    ///
    /// Objects already on the destination are skipped. Copies retain geometry
    /// and object attributes other than layer, while every touched group is
    /// reproduced with the copied subset under a fresh automatic group name.
    /// Rhino's `CopyToLayer` leaves the original selection unchanged and does
    /// not select the copies, including copies on ordinary destination layers.
    pub fn copy_objects_to_layer(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        layer_id: LayerId,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        self.layer_index(layer_id)?;
        let staged = self
            .editable_layer_object_indices(ids)?
            .into_iter()
            .filter(|index| self.objects[*index].attributes.layer_id != layer_id)
            .collect::<Vec<_>>();
        if staged.is_empty() {
            return Ok(Vec::new());
        }
        self.validate_memberships_at_indices(&staged)?;
        self.objects
            .try_reserve_exact(staged.len())
            .map_err(|_| DocumentError::TooManyObjectCopies)?;

        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Copy objects to layer")?;
        }
        let mut copied_ids = Vec::with_capacity(staged.len());
        let mut copied_indices = Vec::with_capacity(staged.len());
        for source_index in staged {
            let original = &self.objects[source_index];
            let id = ObjectId::new();
            let index = self.objects.len();
            let mut attributes = original.attributes.clone();
            attributes.layer_id = layer_id;
            self.objects.push(Object {
                id,
                geometry: original.geometry.clone(),
                attributes,
                isolation: ObjectIsolation::None,
                group_ids: Vec::new(),
            });
            self.record_edit(
                "Copy object to layer",
                Edit::ObjectInserted {
                    index,
                    id,
                    stored: None,
                    selected: false,
                },
            );
            copied_indices.push((source_index, index));
            copied_ids.push(id);
        }
        self.copy_group_memberships(
            &copied_indices,
            true,
            &mut super::groups::GroupNames::default(),
        )?;
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(copied_ids)
    }

    // Validate every source before skipping same-layer no-ops. Indices stay
    // valid while copies append; no geometry is cloned merely for validation.
    fn editable_layer_object_indices(
        &self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<Vec<usize>, DocumentError> {
        let indices = self.resolve_object_indices(ids)?;
        for index in &indices {
            self.ensure_object_editable(&self.objects[*index])?;
        }
        Ok(indices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColorRgb, Geometry, ObjectAttributes, SelectionMode};
    use viboceros_geometry::Point3;

    fn point(x: f64) -> Geometry {
        Geometry::Point(Point3::try_new(x, 0.0, 0.0).unwrap())
    }

    #[test]
    #[ignore = "manual large layer transfer timing"]
    fn benchmark_large_layer_transfers() {
        let mut document = Document::default();
        let target = document.add_layer("target", ColorRgb::BLACK).unwrap();
        document.begin_transaction("fixture").unwrap();
        let ids = (0..20_000)
            .map(|i| document.add_geometry(point(i as f64)).unwrap())
            .collect::<Vec<_>>();
        document.commit_transaction().unwrap();
        let start = std::time::Instant::now();
        assert_eq!(
            document
                .set_objects_layer(ids.iter().copied(), target)
                .unwrap(),
            ids.len()
        );
        eprintln!("20k points, set layer: {:?}", start.elapsed());
        let start = std::time::Instant::now();
        let copied = document
            .copy_objects_to_layer(ids, document.current_layer_id())
            .unwrap();
        eprintln!("20k points, copy to layer: {:?}", start.elapsed());
        assert_eq!(copied.len(), 20_000);
        for (i, object) in document.objects.iter().enumerate() {
            assert_eq!(object.geometry, point((i % 20_000) as f64));
            assert_eq!(
                object.attributes.layer_id,
                if i < 20_000 {
                    target
                } else {
                    document.current_layer_id()
                }
            );
        }
    }

    #[test]
    fn layer_transfer_validation_and_noops_preserve_complete_state() {
        let mut fixture = Document::default();
        let target = fixture.add_layer("target", ColorRgb::BLACK).unwrap();
        let first = fixture.add_geometry(point(0.)).unwrap();
        let already_there = fixture
            .add_geometry_with_attributes(point(1.), ObjectAttributes::on_layer(target))
            .unwrap();
        fixture.set_objects_locked([already_there], true).unwrap();
        fixture
            .select_objects_direct([first], SelectionMode::Replace)
            .unwrap();
        fixture.add_geometry(point(2.)).unwrap();
        fixture.undo().unwrap();
        let missing = ObjectId::new();
        let missing_layer = LayerId::new();
        for copy in [false, true] {
            for (ids, layer, expected) in [
                (
                    vec![first, missing, already_there],
                    missing_layer,
                    Err(DocumentError::LayerNotFound(missing_layer)),
                ),
                (
                    vec![first, missing, already_there],
                    target,
                    Err(DocumentError::ObjectNotFound(missing)),
                ),
                (
                    vec![first, already_there, first],
                    target,
                    Err(DocumentError::ObjectLocked(already_there)),
                ),
                (vec![first, first], fixture.current_layer_id(), Ok(0)),
                (vec![], target, Ok(0)),
            ] {
                let mut document = fixture.clone();
                let before = format!("{document:?}");
                let result = if copy {
                    document.copy_objects_to_layer(ids, layer).map(|v| v.len())
                } else {
                    document.set_objects_layer(ids, layer)
                };
                assert_eq!(result, expected, "copy={copy}");
                assert_eq!(format!("{document:?}"), before, "copy={copy}");
            }
        }
    }

    #[test]
    fn layer_transfer_deduplicates_in_table_order_and_rolls_back() {
        for copy in [false, true] {
            let mut document = Document::default();
            let target = document.add_layer("target", ColorRgb::BLACK).unwrap();
            let ids = (0..40)
                .map(|i| {
                    document.add_geometry_with_attributes(
                        point(i as f64),
                        ObjectAttributes::on_layer(if i % 3 == 0 {
                            target
                        } else {
                            document.current_layer_id()
                        }),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            for id in ids.iter().rev() {
                document
                    .select_objects_direct([*id], SelectionMode::Add)
                    .unwrap();
            }
            let before = document.clone();
            let request = ids
                .iter()
                .rev()
                .flat_map(|id| [*id, *id])
                .collect::<Vec<_>>();
            document.begin_transaction("transfer").unwrap();
            if copy {
                let copies = document.copy_objects_to_layer(request, target).unwrap();
                let expected = (0..40)
                    .filter(|i| i % 3 != 0)
                    .map(|i| point(i as f64))
                    .collect::<Vec<_>>();
                assert_eq!(
                    copies
                        .iter()
                        .map(|id| document.object(*id).unwrap().geometry.clone())
                        .collect::<Vec<_>>(),
                    expected
                );
                assert!(copies.iter().all(|id| !document.is_selected(*id)));
            } else {
                assert_eq!(document.set_objects_layer(request, target).unwrap(), 26);
                assert!(
                    document
                        .objects
                        .iter()
                        .all(|o| o.attributes.layer_id == target)
                );
            }
            assert_eq!(document.selection_order, before.selection_order);
            document.rollback_transaction().unwrap();
            assert_eq!(document.objects, before.objects);
            assert_eq!(document.groups, before.groups);
            assert_eq!(document.selection_order, before.selection_order);
            assert_eq!(document.previous_selection, before.previous_selection);
            assert_eq!(
                document.previous_selection_order,
                before.previous_selection_order
            );
        }
    }

    #[test]
    fn changes_layers_without_replacing_objects_or_groups() {
        let mut document = Document::default();
        let default = document.current_layer_id();
        let target = document
            .add_layer("Target", ColorRgb::new(10, 20, 30))
            .unwrap();
        let hidden = document
            .add_layer("Hidden", ColorRgb::new(40, 50, 60))
            .unwrap();
        let first = document
            .add_geometry_with_attributes(
                point(0.0),
                ObjectAttributes::on_layer(default).with_name("First"),
            )
            .unwrap();
        let second = document.add_geometry(point(1.0)).unwrap();
        let group = document
            .add_group(Some("Assembly".to_owned()), [first, second])
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();

        assert_eq!(
            document.set_objects_layer([first, second], target).unwrap(),
            2
        );
        assert_eq!(document.current_layer_id(), default);
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(document.group(group).unwrap().members().len(), 2);
        assert_eq!(
            document.object(first).unwrap().attributes().name(),
            Some("First")
        );
        assert!(
            [first, second].into_iter().all(|id| document
                .object(id)
                .unwrap()
                .attributes()
                .layer_id()
                == target)
        );
        assert_eq!(
            document.set_objects_layer([first, second], target).unwrap(),
            0
        );

        document.undo().unwrap();
        assert!(
            [first, second].into_iter().all(|id| document
                .object(id)
                .unwrap()
                .attributes()
                .layer_id()
                == default)
        );
        document.redo().unwrap();
        assert!(
            [first, second].into_iter().all(|id| document
                .object(id)
                .unwrap()
                .attributes()
                .layer_id()
                == target)
        );

        document.set_layer_visibility(hidden, false).unwrap();
        assert_eq!(
            document.set_objects_layer([first, second], hidden).unwrap(),
            2
        );
        assert_eq!(document.selected_object_count(), 0);
        assert_eq!(document.group(group).unwrap().members().len(), 2);
    }

    #[test]
    fn copies_only_cross_layer_members_with_automatic_group_names() {
        let mut document = Document::default();
        let default = document.current_layer_id();
        let target = document
            .add_layer("Target", ColorRgb::new(10, 20, 30))
            .unwrap();
        let first = document
            .add_geometry_with_attributes(
                point(0.0),
                ObjectAttributes::on_layer(target).with_name("Already there"),
            )
            .unwrap();
        let second = document
            .add_geometry_with_attributes(
                point(1.0),
                ObjectAttributes::on_layer(default).with_name("Copy me"),
            )
            .unwrap();
        document
            .add_group(Some("Assembly".to_owned()), [first, second])
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();

        let copies = document
            .copy_objects_to_layer([first, second], target)
            .unwrap();
        assert_eq!(copies.len(), 1);
        let copy = document.object(copies[0]).unwrap();
        assert_eq!(copy.attributes().layer_id(), target);
        assert_eq!(copy.attributes().name(), Some("Copy me"));
        assert_eq!(copy.geometry(), document.object(second).unwrap().geometry());
        assert_eq!(document.current_layer_id(), default);
        assert!(document.is_selected(first));
        assert!(document.is_selected(second));
        assert!(!document.is_selected(copies[0]));
        assert_eq!(
            document.group_by_name("Group01").unwrap().members().len(),
            1
        );

        document.undo().unwrap();
        assert!(document.object(copies[0]).is_none());
        assert!(document.group_by_name("Group01").is_none());
        assert_eq!(document.objects().len(), 2);
        document.redo().unwrap();
        assert!(document.object(copies[0]).is_some());
        assert_eq!(
            document.group_by_name("Group01").unwrap().members().len(),
            1
        );
    }

    #[test]
    fn accepts_non_selectable_destinations_and_rejects_invalid_inputs_atomically() {
        let mut document = Document::default();
        let default = document.current_layer_id();
        let hidden = document
            .add_layer("Hidden", ColorRgb::new(10, 20, 30))
            .unwrap();
        let locked = document
            .add_layer("Locked", ColorRgb::new(40, 50, 60))
            .unwrap();
        document.set_layer_visibility(hidden, false).unwrap();
        document.set_layer_locked(locked, true).unwrap();
        let first = document.add_geometry(point(0.0)).unwrap();
        let second = document.add_geometry(point(1.0)).unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();

        let hidden_copy = document.copy_objects_to_layer([first], hidden).unwrap()[0];
        let locked_copy = document.copy_objects_to_layer([second], locked).unwrap()[0];
        assert_eq!(
            document
                .object(hidden_copy)
                .unwrap()
                .attributes()
                .layer_id(),
            hidden
        );
        assert_eq!(
            document
                .object(locked_copy)
                .unwrap()
                .attributes()
                .layer_id(),
            locked
        );
        assert!(document.is_selected(first));
        assert!(!document.is_selected(hidden_copy));
        assert!(!document.is_selected(locked_copy));

        let before = document.objects().cloned().collect::<Vec<_>>();
        let missing_object = ObjectId::new();
        assert_eq!(
            document.set_objects_layer([first, missing_object], hidden),
            Err(DocumentError::ObjectNotFound(missing_object))
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        let missing_layer = LayerId::new();
        assert_eq!(
            document.copy_objects_to_layer([first], missing_layer),
            Err(DocumentError::LayerNotFound(missing_layer))
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            document.object(first).unwrap().attributes().layer_id(),
            default
        );

        document.set_objects_locked([second], true).unwrap();
        let before_locked_failure = document.objects().cloned().collect::<Vec<_>>();
        assert_eq!(
            document.copy_objects_to_layer([first, second], hidden),
            Err(DocumentError::ObjectLocked(second))
        );
        assert_eq!(
            document.objects().cloned().collect::<Vec<_>>(),
            before_locked_failure
        );
    }
}
