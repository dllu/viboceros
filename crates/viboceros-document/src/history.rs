use super::{Document, DocumentError, Group, GroupId, Layer, LayerId, Object, ObjectId};

pub(super) const HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug)]
pub(super) struct HistoryEntry {
    pub label: String,
    pub edits: Vec<Edit>,
}

#[derive(Clone, Debug)]
pub(super) struct PendingTransaction {
    pub label: String,
    pub edits: Vec<Edit>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct History {
    pub undo: Vec<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
    pub active: Option<PendingTransaction>,
}

#[derive(Clone, Debug)]
pub(super) enum Edit {
    ObjectInserted {
        index: usize,
        id: ObjectId,
        stored: Option<Object>,
    },
    ObjectRemoved {
        index: usize,
        id: ObjectId,
        stored: Option<Object>,
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
    GroupMemberRemoved {
        group_id: GroupId,
        object_id: ObjectId,
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
            Self::ObjectInserted { index, id, stored } => {
                ensure_empty(stored, "inserted object was already stored")?;
                *stored = Some(remove_object(document, *index, *id)?);
            }
            Self::ObjectRemoved { index, stored, .. } => {
                let object = stored.take().ok_or(DocumentError::HistoryInvariant(
                    "removed object was not stored",
                ))?;
                insert_at(&mut document.objects, *index, object)?;
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
            Self::GroupMemberRemoved {
                group_id,
                object_id,
            } => {
                let group = document
                    .groups
                    .iter_mut()
                    .find(|group| group.id == *group_id)
                    .ok_or(DocumentError::HistoryInvariant(
                        "group for restored member was missing",
                    ))?;
                if !group.members.insert(*object_id) {
                    return Err(DocumentError::HistoryInvariant(
                        "restored group member already existed",
                    ));
                }
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
            Self::ObjectInserted { index, stored, .. } => {
                let object = stored.take().ok_or(DocumentError::HistoryInvariant(
                    "inserted object was not stored",
                ))?;
                insert_at(&mut document.objects, *index, object)?;
            }
            Self::ObjectRemoved { index, id, stored } => {
                ensure_empty(stored, "removed object was already stored")?;
                *stored = Some(remove_object(document, *index, *id)?);
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
            Self::GroupMemberRemoved {
                group_id,
                object_id,
            } => {
                let group = document
                    .groups
                    .iter_mut()
                    .find(|group| group.id == *group_id)
                    .ok_or(DocumentError::HistoryInvariant(
                        "group for removed member was missing",
                    ))?;
                if !group.members.remove(object_id) {
                    return Err(DocumentError::HistoryInvariant(
                        "removed group member was missing",
                    ));
                }
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
