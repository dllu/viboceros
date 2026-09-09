//! Batch removal and linear object-order replay without geometry snapshots.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct RemovedObjects {
    pub ids: BTreeSet<ObjectId>,
    positions: Vec<usize>,
    object_count: usize,
    stored: Option<Vec<Object>>,
    selected: Vec<ObjectId>,
}

impl Document {
    /// Deletes exactly these objects atomically, coalescing duplicate IDs.
    /// Empty group definitions survive; Undo restores object order and memberships.
    /// Like `delete_object`, explicit IDs may include hidden or locked objects.
    pub fn delete_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        let ids = ids.into_iter().collect::<BTreeSet<_>>();
        if ids.is_empty() {
            return Ok(0);
        }
        let positions = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| ids.contains(&o.id))
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if positions.len() != ids.len() {
            return Err(DocumentError::ObjectNotFound(
                *ids.iter().find(|id| self.object(**id).is_none()).unwrap(),
            ));
        }
        let count = ids.len();
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Delete objects")?;
        }
        let mut removed = RemovedObjects {
            ids,
            positions,
            object_count: self.objects.len(),
            stored: None,
            selected: vec![],
        };
        if let Err(error) = removed.remove(self) {
            if owns_transaction {
                self.rollback_transaction()?;
            }
            return Err(error);
        }
        self.record_edit("Delete objects", Edit::ObjectsRemoved(Box::new(removed)));
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(count)
    }
}

impl RemovedObjects {
    pub(super) fn remove(&mut self, document: &mut Document) -> Result<(), DocumentError> {
        if self.stored.is_some()
            || document.objects.len() != self.object_count
            || self.positions.iter().any(|i| {
                document
                    .objects
                    .get(*i)
                    .is_none_or(|o| !self.ids.contains(&o.id))
            })
        {
            return Err(DocumentError::HistoryInvariant(
                "batch deletion source mismatch",
            ));
        }
        let mut kept = Vec::with_capacity(self.object_count - self.ids.len());
        let mut removed = Vec::with_capacity(self.ids.len());
        for object in std::mem::take(&mut document.objects) {
            if self.ids.contains(&object.id) {
                removed.push(object);
            } else {
                kept.push(object);
            }
        }
        document.objects = kept;
        for group in &mut document.groups {
            group.members.retain(|id| !self.ids.contains(id));
        }
        self.selected = document
            .selection_order
            .iter()
            .copied()
            .filter(|id| self.ids.contains(id) && document.selection.contains(id))
            .collect();
        document.selection.retain(|id| !self.ids.contains(id));
        document.selection_order.retain(|id| !self.ids.contains(id));
        self.stored = Some(removed);
        Ok(())
    }

    pub(super) fn restore(&mut self, document: &mut Document) -> Result<(), DocumentError> {
        let Some(stored) = self.stored.as_ref() else {
            return Err(DocumentError::HistoryInvariant(
                "batch deletion objects missing",
            ));
        };
        let group_ids = document
            .groups
            .iter()
            .map(|g| g.id)
            .collect::<BTreeSet<_>>();
        if document.objects.len() + stored.len() != self.object_count
            || document.objects.iter().any(|o| self.ids.contains(&o.id))
            || stored
                .iter()
                .any(|o| o.group_ids.iter().any(|g| !group_ids.contains(g)))
        {
            return Err(DocumentError::HistoryInvariant(
                "batch deletion restoration mismatch",
            ));
        }
        let mut memberships: BTreeMap<GroupId, Vec<ObjectId>> = BTreeMap::new();
        for object in stored {
            for group in &object.group_ids {
                memberships.entry(*group).or_default().push(object.id);
            }
        }
        let mut removed = self.stored.take().unwrap().into_iter();
        let mut survivors = std::mem::take(&mut document.objects).into_iter();
        let mut positions = self.positions.iter().copied().peekable();
        let mut output = Vec::with_capacity(self.object_count);
        for index in 0..self.object_count {
            if positions.peek() == Some(&index) {
                positions.next();
                output.push(removed.next().expect("validated removed object count"));
            } else {
                output.push(survivors.next().expect("validated surviving object count"));
            }
        }
        document.objects = output;
        for group in &mut document.groups {
            if let Some(ids) = memberships.remove(&group.id) {
                group.members.extend(ids);
            }
        }
        // Restore membership and relative pick order of removed objects without
        // replacing unrelated selection made after deletion.
        for id in &self.selected {
            if document.selection.insert(*id) {
                document.selection_order.push(*id);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
