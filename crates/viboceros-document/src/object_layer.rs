use std::collections::{BTreeMap, BTreeSet};

use super::history::Edit;
use super::{Document, DocumentError, Group, GroupId, LayerId, Object, ObjectId, ObjectIsolation};

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
            .stage_editable_layer_objects(ids)?
            .into_iter()
            .filter_map(|(index, before)| {
                if before.attributes.layer_id == layer_id {
                    return None;
                }
                let mut after = before.clone();
                after.attributes.layer_id = layer_id;
                Some((index, before, after))
            })
            .collect::<Vec<_>>();
        if staged.is_empty() {
            return Ok(0);
        }

        let changed_count = staged.len();
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Set object layer")?;
        }
        for (index, before, after) in staged {
            let id = before.id;
            self.objects[index] = after.clone();
            self.record_edit(
                "Set object layer",
                Edit::ObjectChanged { id, before, after },
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
            .stage_editable_layer_objects(ids)?
            .into_iter()
            .filter(|(_, object)| object.attributes.layer_id != layer_id)
            .collect::<Vec<_>>();
        if staged.is_empty() {
            return Ok(Vec::new());
        }

        let originals = staged
            .iter()
            .map(|(_, object)| object.id)
            .collect::<BTreeSet<_>>();
        let copied_groups = self
            .groups
            .iter()
            .filter_map(|group| {
                let members = group
                    .members
                    .iter()
                    .filter(|member| originals.contains(member))
                    .copied()
                    .collect::<Vec<_>>();
                (!members.is_empty()).then_some(members)
            })
            .collect::<Vec<_>>();

        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Copy objects to layer")?;
        }
        let mut copied_ids = Vec::with_capacity(staged.len());
        let mut copied_by_original = BTreeMap::new();
        for (_, original) in staged {
            let id = ObjectId::new();
            let index = self.objects.len();
            let mut attributes = original.attributes;
            attributes.layer_id = layer_id;
            self.objects.push(Object {
                id,
                geometry: original.geometry,
                attributes,
                isolation: ObjectIsolation::None,
            });
            self.record_edit(
                "Copy object to layer",
                Edit::ObjectInserted {
                    index,
                    id,
                    stored: None,
                },
            );
            copied_by_original.insert(original.id, id);
            copied_ids.push(id);
        }
        for original_members in copied_groups {
            let members = original_members
                .into_iter()
                .map(|member| copied_by_original[&member])
                .collect();
            let id = GroupId::new();
            let index = self.groups.len();
            self.groups.push(Group {
                id,
                name: Some(self.next_unused_group_name()),
                members,
            });
            self.record_edit(
                "Copy group to layer",
                Edit::GroupInserted {
                    index,
                    id,
                    stored: None,
                },
            );
        }
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(copied_ids)
    }

    fn stage_editable_layer_objects(
        &self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<Vec<(usize, Object)>, DocumentError> {
        let ids = ids.into_iter().collect::<BTreeSet<_>>();
        if let Some(missing) = ids.iter().find(|id| self.object(**id).is_none()) {
            return Err(DocumentError::ObjectNotFound(*missing));
        }

        let mut staged = Vec::with_capacity(ids.len());
        for (index, object) in self.objects.iter().enumerate() {
            if !ids.contains(&object.id) {
                continue;
            }
            if object.attributes.locked {
                return Err(DocumentError::ObjectLocked(object.id));
            }
            let layer = self
                .layer(object.attributes.layer_id)
                .ok_or(DocumentError::LayerNotFound(object.attributes.layer_id))?;
            if layer.locked {
                return Err(DocumentError::LayerLocked(layer.id));
            }
            staged.push((index, object.clone()));
        }
        Ok(staged)
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
