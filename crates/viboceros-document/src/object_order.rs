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

/// Applies the stable partition or its inverse in linear time with one index
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
    if objects.len() != count || moved.len() > count || moved.windows(2).any(|p| p[0].0 >= p[1].0) {
        return Err(invalid());
    }
    let tail = count - moved.len();
    for (i, (index, id)) in moved.iter().enumerate() {
        if *index >= count || objects[if forward { *index } else { tail + i }].id != *id {
            return Err(invalid());
        }
    }
    let mut destinations = vec![0; count];
    let (mut picked, mut retained) = (0, 0);
    for original in 0..count {
        let destination = if moved.get(picked).is_some_and(|(i, _)| *i == original) {
            let d = tail + picked;
            picked += 1;
            d
        } else {
            let d = retained;
            retained += 1;
            d
        };
        if forward {
            destinations[original] = destination;
        } else {
            destinations[destination] = original;
        }
    }
    for i in 0..count {
        while destinations[i] != i {
            let j = destinations[i];
            objects.swap(i, j);
            destinations.swap(i, j);
        }
    }
    Ok(())
}
