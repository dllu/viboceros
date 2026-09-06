//! Chronological object renewal without cloning geometry into ordering history.
use super::*;

#[cfg(test)]
mod tests;

impl Document {
    /// Moves the specified objects after all others, preserving their relative
    /// document order, identities, attributes, groups, and selection action order.
    /// Duplicate IDs are coalesced; missing IDs fail before any mutation.
    /// Intended for replacement commands that renew an object's creation order.
    pub fn move_objects_to_end(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<bool, DocumentError> {
        let ids = ids.into_iter().collect::<BTreeSet<_>>();
        let moved = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| ids.contains(&o.id))
            .map(|(i, o)| (i, o.id))
            .collect::<Vec<_>>();
        if moved.len() != ids.len() {
            return Err(DocumentError::ObjectNotFound(
                *ids.iter()
                    .find(|id| self.object(**id).is_none())
                    .expect("missing requested object"),
            ));
        }
        self.finish_order_change(moved)
    }

    /// Renews objects in the caller's explicit order, retaining the relative
    /// order of all untouched objects. Duplicate IDs keep their first position.
    /// Identity, geometry, memberships, and selection action order are unchanged.
    pub fn move_objects_to_end_in_order(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<bool, DocumentError> {
        let mut ranks = BTreeMap::new();
        for id in ids {
            let rank = ranks.len();
            ranks.entry(id).or_insert(rank);
        }
        let mut moved = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| ranks.contains_key(&o.id))
            .map(|(i, o)| (i, o.id))
            .collect::<Vec<_>>();
        if moved.len() != ranks.len() {
            return Err(DocumentError::ObjectNotFound(
                *ranks
                    .keys()
                    .find(|id| self.object(**id).is_none())
                    .expect("missing requested object"),
            ));
        }
        moved.sort_unstable_by_key(|(_, id)| ranks[id]);
        self.finish_order_change(moved)
    }

    fn finish_order_change(
        &mut self,
        moved: Vec<(usize, ObjectId)>,
    ) -> Result<bool, DocumentError> {
        let count = self.objects.len();
        let tail = count - moved.len();
        if moved
            .iter()
            .enumerate()
            .all(|(i, (index, _))| *index == tail + i)
        {
            return Ok(false);
        }
        apply(&mut self.objects, &moved, count, true)?;
        self.record_edit(
            "Renew object order",
            Edit::ObjectsMovedToEnd {
                moved,
                object_count: count,
            },
        );
        Ok(true)
    }
}

/// Applies an ordered tail move or its inverse in linear time with one index
/// vector. History stores only the moved IDs and original indices, not geometry
/// or a complete document permutation. Validate before the first object swap.
pub(super) fn apply(
    objects: &mut [Object],
    moved: &[(usize, ObjectId)],
    count: usize,
    forward: bool,
) -> Result<(), DocumentError> {
    let invalid =
        || DocumentError::HistoryInvariant("object-order history does not match document");
    if objects.len() != count || moved.len() > count {
        return Err(invalid());
    }
    let tail = count - moved.len();
    let mut destinations = vec![usize::MAX; count];
    for (i, (index, id)) in moved.iter().enumerate() {
        if *index >= count
            || destinations[*index] != usize::MAX
            || objects[if forward { *index } else { tail + i }].id != *id
        {
            return Err(invalid());
        }
        destinations[*index] = tail + i;
    }
    let mut retained = 0;
    for destination in &mut destinations {
        if *destination == usize::MAX {
            *destination = retained;
            retained += 1;
        }
    }
    for i in 0..count {
        if forward {
            while destinations[i] != i {
                let j = destinations[i];
                objects.swap(i, j);
                destinations.swap(i, j);
            }
        } else {
            // Traverse each cycle with adjacent swaps instead of anchor swaps
            // to apply its inverse without allocating another permutation.
            let mut cursor = i;
            while destinations[cursor] != i {
                let next = destinations[cursor];
                objects.swap(cursor, next);
                destinations[cursor] = cursor;
                cursor = next;
            }
            destinations[cursor] = cursor;
        }
    }
    Ok(())
}
