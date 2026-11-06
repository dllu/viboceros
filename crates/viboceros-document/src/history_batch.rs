//! Coalesce completed commands while each getter keeps its own local checkpoints.
use super::{Document, DocumentError, history::HISTORY_LIMIT};
use uuid::Uuid;

/// Explicit begin/checkpoint/finish token; no transaction stays open between inputs.
/// Checkpoint only after a child command has finished using its HistoryGroup.
pub struct HistoryBatch {
    id: Uuid,
    label: String,
    base: Vec<(Uuid, usize)>,
}
impl Document {
    pub fn begin_history_batch(
        &mut self,
        label: impl Into<String>,
    ) -> Result<HistoryBatch, DocumentError> {
        self.ensure_no_transaction()?;
        let id = Uuid::new_v4();
        let label = label.into();
        let base = self
            .history
            .undo
            .iter()
            .map(|e| (e.id, e.edits.len()))
            .collect();
        self.history.batches.push(id);
        Ok(HistoryBatch {
            id,
            label: if label.trim().is_empty() {
                "Macro".into()
            } else {
                label
            },
            base,
        })
    }
    pub fn history_batch_is_current(&self, batch: &HistoryBatch) -> bool {
        self.history.batches.contains(&batch.id)
            && self.history.undo.len() >= batch.base.len()
            && self
                .history
                .undo
                .iter()
                .zip(&batch.base)
                .all(|(e, (id, n))| e.id == *id && e.edits.len() == *n)
    }
    pub fn checkpoint_history_batch(
        &mut self,
        batch: &HistoryBatch,
    ) -> Result<bool, DocumentError> {
        self.ensure_no_transaction()?;
        if self.history.batches.last() != Some(&batch.id) || !self.history_batch_is_current(batch) {
            return Err(DocumentError::HistoryBatchStale);
        }
        if self.history.undo.len() == batch.base.len() {
            return Ok(false);
        }
        let mut entries = self.history.undo.split_off(batch.base.len()).into_iter();
        let mut combined = entries.next().unwrap();
        let changed = combined.id != batch.id || entries.len() != 0;
        for entry in entries {
            combined.edits.extend(entry.edits);
            combined.object_ids.extend(entry.object_ids);
            combined.updates_last_changed_objects |= entry.updates_last_changed_objects;
        }
        combined.id = batch.id;
        combined.label = batch.label.clone();
        self.history.undo.push(combined);
        if changed {
            self.history.version = Uuid::new_v4();
        }
        Ok(changed)
    }
    pub fn finish_history_batch(&mut self, batch: &HistoryBatch) -> Result<bool, DocumentError> {
        self.ensure_no_transaction()?;
        if self.history.batches.last() != Some(&batch.id) {
            return Err(DocumentError::HistoryBatchStale);
        }
        let result = self.checkpoint_history_batch(batch);
        self.history.batches.pop();
        if self.history.batches.is_empty() && self.history.undo.len() > HISTORY_LIMIT {
            self.history
                .undo
                .drain(..self.history.undo.len() - HISTORY_LIMIT);
        }
        result
    }
}

#[cfg(test)]
mod tests;
