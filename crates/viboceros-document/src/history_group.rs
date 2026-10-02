//! Incrementally committed commands with one external Undo and local checkpoints.
use super::*;

#[cfg(test)]
mod tests;

/// Owned continuation token. No transaction stays open between accepted picks.
/// Dropping it finishes the command; committed edits remain in ordinary history.
#[derive(Debug)]
pub struct HistoryGroup {
    id: Uuid,
    version: Uuid,
    label: String,
    checkpoints: Vec<Checkpoint>,
    last_changed_before: BTreeSet<ObjectId>,
}

#[derive(Debug)]
struct Checkpoint {
    edit_start: usize,
    added_ids: BTreeSet<ObjectId>,
    affected_ids: BTreeSet<ObjectId>,
    updated_last_before: bool,
}

impl HistoryGroup {
    pub fn can_undo(&self) -> bool {
        !self.checkpoints.is_empty()
    }
}

impl Document {
    pub fn begin_history_group(
        &self,
        label: impl Into<String>,
    ) -> Result<HistoryGroup, DocumentError> {
        self.ensure_no_transaction()?;
        let label = label.into();
        Ok(HistoryGroup {
            id: Uuid::new_v4(),
            version: self.history.version,
            label: if label.trim().is_empty() {
                "Edit".into()
            } else {
                label
            },
            checkpoints: Vec::new(),
            last_changed_before: self.last_changed_objects.clone(),
        })
    }

    pub fn history_group_is_current(&self, group: &HistoryGroup) -> bool {
        self.history.version == group.version
            && (!group.can_undo()
                || self
                    .history
                    .undo
                    .last()
                    .is_some_and(|entry| entry.id == group.id))
    }

    pub fn begin_group_transaction(&mut self, group: &HistoryGroup) -> Result<(), DocumentError> {
        self.ensure_no_transaction()?;
        if !self.history_group_is_current(group) {
            return Err(DocumentError::HistoryGroupStale);
        }
        self.begin_transaction(group.label.clone())?;
        self.history.active.as_mut().unwrap().group = Some(group.id);
        Ok(())
    }

    /// Atomically accepts one step and appends it to the group's existing entry.
    /// No-op steps preserve Redo and create no checkpoint.
    pub fn commit_group_transaction(
        &mut self,
        group: &mut HistoryGroup,
    ) -> Result<bool, DocumentError> {
        if !self.history_group_is_current(group)
            || self
                .history
                .active
                .as_ref()
                .is_none_or(|transaction| transaction.group != Some(group.id))
        {
            return Err(DocumentError::HistoryGroupStale);
        }
        let transaction = self.history.active.take().unwrap();
        if transaction.selection_before.is_subset(&self.selection) {
            self.previous_selection = transaction.previous_selection_before;
            self.previous_selection_order = transaction.previous_selection_order_before;
        } else {
            self.previous_selection = transaction.selection_before;
            self.previous_selection_order = transaction.selection_order_before;
        }
        if transaction.edits.is_empty() {
            return Ok(false);
        }
        let mut entry = if group.can_undo() {
            self.history.undo.pop().unwrap()
        } else {
            HistoryEntry {
                id: group.id,
                label: group.label.clone(),
                edits: Vec::new(),
                object_ids: BTreeSet::new(),
                updates_last_changed_objects: false,
            }
        };
        group.checkpoints.push(Checkpoint {
            edit_start: entry.edits.len(),
            added_ids: transaction
                .object_ids
                .difference(&entry.object_ids)
                .copied()
                .collect(),
            affected_ids: transaction.object_ids.clone(),
            updated_last_before: entry.updates_last_changed_objects,
        });
        entry.edits.extend(transaction.edits);
        entry.object_ids.extend(transaction.object_ids);
        self.push_new_undo(entry);
        group.version = self.history.version;
        Ok(true)
    }

    /// Discards only the last accepted step, leaving earlier picks in the same
    /// external Undo entry. Rejected/stale tokens cannot undo another command.
    pub fn undo_history_group(&mut self, group: &mut HistoryGroup) -> Result<bool, DocumentError> {
        self.ensure_no_transaction()?;
        if !self.history_group_is_current(group) {
            return Err(DocumentError::HistoryGroupStale);
        }
        let Some(checkpoint) = group.checkpoints.last() else {
            return Ok(false);
        };
        let mut entry = self.history.undo.pop().unwrap();
        let mut suffix = HistoryEntry {
            id: group.id,
            label: group.label.clone(),
            edits: entry.edits.split_off(checkpoint.edit_start),
            object_ids: checkpoint.affected_ids.clone(),
            updates_last_changed_objects: false,
        };
        let unchanged = self.selection_untouched_by_history(&suffix);
        let selection_before = self.selection.clone();
        let selection_order_before = self.selection_order.clone();
        for index in (0..suffix.edits.len()).rev() {
            if let Err(error) = suffix.edits[index].undo(self) {
                for restore in index + 1..suffix.edits.len() {
                    suffix.edits[restore].redo(self)?;
                }
                entry.edits.extend(suffix.edits);
                self.history.undo.push(entry);
                self.selection = selection_before;
                self.selection_order = selection_order_before;
                return Err(error);
            }
        }
        for id in &checkpoint.added_ids {
            entry.object_ids.remove(id);
        }
        entry.updates_last_changed_objects = checkpoint.updated_last_before;
        group.checkpoints.pop();
        if group.can_undo() {
            if entry.updates_last_changed_objects {
                self.update_last_changed_objects(&entry);
            } else {
                self.last_changed_objects = group.last_changed_before.clone();
            }
            self.history.undo.push(entry);
        } else {
            self.last_changed_objects = group.last_changed_before.clone();
        }
        self.prune_selection_after_history_preserving(&unchanged);
        self.history.version = Uuid::new_v4();
        group.version = self.history.version;
        Ok(true)
    }
}
