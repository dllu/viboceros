use std::collections::BTreeSet;

use super::{Document, DocumentError, Group, GroupId, Layer, LayerId, Object, ObjectId};

#[cfg(test)]
mod tests;

pub(super) const HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug)]
pub(super) struct HistoryEntry {
    pub label: String,
    pub edits: Vec<Edit>,
    pub object_ids: BTreeSet<ObjectId>,
    /// Pure removal entries preserve SelLast, including when replayed.
    pub updates_last_changed_objects: bool,
}

#[derive(Clone, Debug)]
pub(super) struct PendingTransaction {
    pub label: String,
    pub edits: Vec<Edit>,
    pub object_ids: BTreeSet<ObjectId>,
    pub selection_before: BTreeSet<ObjectId>,
    pub selection_order_before: Vec<ObjectId>,
    pub previous_selection_before: BTreeSet<ObjectId>,
    pub previous_selection_order_before: Vec<ObjectId>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct History {
    pub undo: Vec<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
    pub active: Option<PendingTransaction>,
}

#[derive(Clone, Debug)]
pub(super) enum Edit {
    ToleranceChanged {
        tolerance: viboceros_geometry::Tolerance,
    },
    UnitsChanged {
        units: viboceros_geometry::LengthUnitSystem,
        tolerance: viboceros_geometry::Tolerance,
        geometries: Option<Vec<(ObjectId, super::Geometry)>>,
    },
    ObjectInserted {
        index: usize,
        id: ObjectId,
        stored: Option<Object>,
        selected: bool,
    },
    ObjectRemoved {
        index: usize,
        id: ObjectId,
        stored: Option<Object>,
        selected: bool,
    },
    ObjectChanged {
        id: ObjectId,
        selected: bool,
        /// Keep large before/after snapshots out of every small history edit.
        states: Box<[Object; 2]>,
    },
    ObjectsMovedToEnd {
        moved: Vec<(usize, ObjectId)>,
        object_count: usize,
    },
    LayerInserted {
        index: usize,
        id: LayerId,
        stored: Option<Layer>,
    },
    LayerRemoved {
        index: usize,
        id: LayerId,
        stored: Option<Layer>,
    },
    LayerChanged {
        id: LayerId,
        before: Layer,
        after: Layer,
    },
    GroupInserted {
        index: usize,
        id: GroupId,
        stored: Option<Group>,
    },
    GroupRemoved {
        index: usize,
        id: GroupId,
        stored: Option<Group>,
    },
    ObjectGroupsChanged {
        id: ObjectId,
        before: Vec<GroupId>,
        after: Vec<GroupId>,
    },
    CurrentLayerChanged {
        before: LayerId,
        after: LayerId,
    },
    ObjectsCleared {
        stored_objects: Vec<Object>,
        stored_groups: Vec<Group>,
    },
}

impl Edit {
    pub fn undo(&mut self, document: &mut Document) -> Result<(), DocumentError> {
        match self {
            Self::ToleranceChanged { tolerance } => {
                std::mem::swap(&mut document.tolerance, tolerance);
            }
            Self::UnitsChanged {
                units,
                tolerance,
                geometries,
            } => {
                super::units::exchange_units(document, units, tolerance, geometries)?;
            }
            Self::ObjectInserted {
                index,
                id,
                stored,
                selected,
            } => {
                ensure_empty(stored, "inserted object was already stored")?;
                *stored = Some(remove_object(document, *index, *id)?);
                exchange_selection(document, *id, selected, false);
            }
            Self::ObjectRemoved {
                index,
                id,
                stored,
                selected,
            } => {
                let object = stored.take().ok_or(DocumentError::HistoryInvariant(
                    "removed object was not stored",
                ))?;
                insert_at(&mut document.objects, *index, object)?;
                exchange_selection(document, *id, selected, true);
            }
            Self::ObjectChanged {
                id,
                states,
                selected,
            } => {
                replace_object(document, *id, &states[1], &states[0])?;
                exchange_selection(document, *id, selected, true);
            }
            Self::ObjectsMovedToEnd {
                moved,
                object_count,
            } => {
                super::object_order::apply(&mut document.objects, moved, *object_count, false)?;
            }
            Self::LayerInserted { index, id, stored } => {
                ensure_empty(stored, "inserted layer was already stored")?;
                *stored = Some(remove_layer(document, *index, *id)?);
            }
            Self::LayerRemoved { index, stored, .. } => {
                let layer = stored.take().ok_or(DocumentError::HistoryInvariant(
                    "removed layer was not stored",
                ))?;
                insert_at(&mut document.layers, *index, layer)?;
            }
            Self::LayerChanged { id, before, after } => {
                replace_layer(document, *id, after, before)?;
            }
            Self::GroupInserted { index, id, stored } => {
                ensure_empty(stored, "inserted group was already stored")?;
                *stored = Some(remove_group(document, *index, *id)?);
            }
            Self::GroupRemoved { index, stored, .. } => {
                let group = stored.take().ok_or(DocumentError::HistoryInvariant(
                    "removed group was not stored",
                ))?;
                insert_at(&mut document.groups, *index, group)?;
            }
            Self::ObjectGroupsChanged { id, before, after } => {
                super::groups::apply_memberships(document, *id, after, before)?;
            }
            Self::CurrentLayerChanged { before, .. } => {
                ensure_layer_exists(document, *before)?;
                document.current_layer = *before;
            }
            Self::ObjectsCleared {
                stored_objects,
                stored_groups,
            } => {
                std::mem::swap(&mut document.objects, stored_objects);
                std::mem::swap(&mut document.groups, stored_groups);
            }
        }
        Ok(())
    }

    pub fn redo(&mut self, document: &mut Document) -> Result<(), DocumentError> {
        match self {
            Self::ToleranceChanged { tolerance } => {
                std::mem::swap(&mut document.tolerance, tolerance);
            }
            Self::UnitsChanged {
                units,
                tolerance,
                geometries,
            } => {
                super::units::exchange_units(document, units, tolerance, geometries)?;
            }
            Self::ObjectInserted {
                index,
                id,
                stored,
                selected,
            } => {
                let object = stored.take().ok_or(DocumentError::HistoryInvariant(
                    "inserted object was not stored",
                ))?;
                insert_at(&mut document.objects, *index, object)?;
                exchange_selection(document, *id, selected, true);
            }
            Self::ObjectRemoved {
                index,
                id,
                stored,
                selected,
            } => {
                ensure_empty(stored, "removed object was already stored")?;
                *stored = Some(remove_object(document, *index, *id)?);
                exchange_selection(document, *id, selected, false);
            }
            Self::ObjectChanged {
                id,
                states,
                selected,
            } => {
                replace_object(document, *id, &states[0], &states[1])?;
                exchange_selection(document, *id, selected, true);
            }
            Self::ObjectsMovedToEnd {
                moved,
                object_count,
            } => {
                super::object_order::apply(&mut document.objects, moved, *object_count, true)?;
            }
            Self::LayerInserted { index, stored, .. } => {
                let layer = stored.take().ok_or(DocumentError::HistoryInvariant(
                    "inserted layer was not stored",
                ))?;
                insert_at(&mut document.layers, *index, layer)?;
            }
            Self::LayerRemoved { index, id, stored } => {
                ensure_empty(stored, "removed layer was already stored")?;
                *stored = Some(remove_layer(document, *index, *id)?);
            }
            Self::LayerChanged { id, before, after } => {
                replace_layer(document, *id, before, after)?;
            }
            Self::GroupInserted { index, stored, .. } => {
                let group = stored.take().ok_or(DocumentError::HistoryInvariant(
                    "inserted group was not stored",
                ))?;
                insert_at(&mut document.groups, *index, group)?;
            }
            Self::GroupRemoved { index, id, stored } => {
                ensure_empty(stored, "removed group was already stored")?;
                *stored = Some(remove_group(document, *index, *id)?);
            }
            Self::ObjectGroupsChanged { id, before, after } => {
                super::groups::apply_memberships(document, *id, before, after)?;
            }
            Self::CurrentLayerChanged { after, .. } => {
                ensure_layer_exists(document, *after)?;
                document.current_layer = *after;
            }
            Self::ObjectsCleared {
                stored_objects,
                stored_groups,
            } => {
                std::mem::swap(&mut document.objects, stored_objects);
                std::mem::swap(&mut document.groups, stored_groups);
            }
        }
        Ok(())
    }
}

// Selection belongs to the stored object state. Exchange it on every replay:
// users may change selection between Undo and Redo. Unrelated IDs are untouched.
fn exchange_selection(document: &mut Document, id: ObjectId, stored: &mut bool, exists: bool) {
    let current = document.selection.contains(&id);
    if *stored && exists {
        if document.selection.insert(id) {
            document.selection_order.push(id);
        }
    } else {
        document.selection.remove(&id);
    }
    // The replay boundary filters/deduplicates order once, avoiding quadratic
    // work when undoing commands with many selected output objects.
    *stored = current;
}

fn ensure_empty<T>(value: &Option<T>, message: &'static str) -> Result<(), DocumentError> {
    if value.is_none() {
        Ok(())
    } else {
        Err(DocumentError::HistoryInvariant(message))
    }
}

fn ensure_layer_exists(document: &Document, id: LayerId) -> Result<(), DocumentError> {
    if document.layers.iter().any(|layer| layer.id == id) {
        Ok(())
    } else {
        Err(DocumentError::HistoryInvariant(
            "history referenced a missing layer",
        ))
    }
}

fn remove_object(
    document: &mut Document,
    index: usize,
    expected_id: ObjectId,
) -> Result<Object, DocumentError> {
    let object = document
        .objects
        .get(index)
        .ok_or(DocumentError::HistoryInvariant(
            "history object index was out of bounds",
        ))?;
    if object.id != expected_id {
        return Err(DocumentError::HistoryInvariant(
            "history object identity did not match",
        ));
    }
    Ok(document.objects.remove(index))
}

fn replace_object(
    document: &mut Document,
    id: ObjectId,
    expected: &Object,
    replacement: &Object,
) -> Result<(), DocumentError> {
    if expected.id != id || replacement.id != id {
        return Err(DocumentError::HistoryInvariant(
            "changed object identity did not match",
        ));
    }
    let object = document
        .objects
        .iter_mut()
        .find(|object| object.id == id)
        .ok_or(DocumentError::HistoryInvariant(
            "changed object was missing",
        ))?;
    if object != expected {
        return Err(DocumentError::HistoryInvariant(
            "changed object state did not match",
        ));
    }
    *object = replacement.clone();
    Ok(())
}

fn remove_layer(
    document: &mut Document,
    index: usize,
    expected_id: LayerId,
) -> Result<Layer, DocumentError> {
    let layer = document
        .layers
        .get(index)
        .ok_or(DocumentError::HistoryInvariant(
            "history layer index was out of bounds",
        ))?;
    if layer.id != expected_id {
        return Err(DocumentError::HistoryInvariant(
            "history layer identity did not match",
        ));
    }
    Ok(document.layers.remove(index))
}

fn replace_layer(
    document: &mut Document,
    id: LayerId,
    expected: &Layer,
    replacement: &Layer,
) -> Result<(), DocumentError> {
    if expected.id != id || replacement.id != id {
        return Err(DocumentError::HistoryInvariant(
            "changed layer identity did not match",
        ));
    }
    let layer = document
        .layers
        .iter_mut()
        .find(|layer| layer.id == id)
        .ok_or(DocumentError::HistoryInvariant("changed layer was missing"))?;
    if layer != expected {
        return Err(DocumentError::HistoryInvariant(
            "changed layer state did not match",
        ));
    }
    *layer = replacement.clone();
    Ok(())
}

fn remove_group(
    document: &mut Document,
    index: usize,
    expected_id: GroupId,
) -> Result<Group, DocumentError> {
    let group = document
        .groups
        .get(index)
        .ok_or(DocumentError::HistoryInvariant(
            "history group index was out of bounds",
        ))?;
    if group.id != expected_id {
        return Err(DocumentError::HistoryInvariant(
            "history group identity did not match",
        ));
    }
    Ok(document.groups.remove(index))
}

fn insert_at<T>(values: &mut Vec<T>, index: usize, value: T) -> Result<(), DocumentError> {
    if index > values.len() {
        return Err(DocumentError::HistoryInvariant(
            "history insertion index was out of bounds",
        ));
    }
    values.insert(index, value);
    Ok(())
}
