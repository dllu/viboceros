//! Ordered object memberships with a synchronized group-to-member index.
use super::*;
mod names;
pub(super) use names::GroupNames;

#[cfg(test)]
mod tests;

impl Document {
    pub fn groups(&self) -> impl ExactSizeIterator<Item = &Group> {
        self.groups.iter()
    }

    pub fn group(&self, id: GroupId) -> Option<&Group> {
        self.groups.iter().find(|group| group.id == id)
    }

    pub fn group_by_name(&self, name: &str) -> Option<&Group> {
        let name = name.trim();
        self.groups
            .iter()
            .find(|group| group.name.as_deref() == Some(name))
    }

    /// Atomically replaces an object's complete ordered membership list.
    /// Existing membership reordering is meaningful; duplicate entries are invalid.
    /// Group edits can include hidden/locked objects, as can file import.
    pub fn set_object_group_memberships(
        &mut self,
        id: ObjectId,
        groups: impl IntoIterator<Item = GroupId>,
    ) -> Result<bool, DocumentError> {
        let index = self
            .objects
            .iter()
            .position(|object| object.id == id)
            .ok_or(DocumentError::ObjectNotFound(id))?;
        self.set_object_group_memberships_at(index, groups)
    }

    // The caller owns a validated index and must not reorder/remove objects.
    pub(super) fn set_object_group_memberships_at(
        &mut self,
        index: usize,
        groups: impl IntoIterator<Item = GroupId>,
    ) -> Result<bool, DocumentError> {
        let id = self.objects[index].id;
        let before = self.objects[index].group_ids.clone();
        let after = groups.into_iter().collect::<Vec<_>>();
        let mut unique = BTreeSet::new();
        for group in &after {
            if self.group(*group).is_none() {
                return Err(DocumentError::GroupNotFound(*group));
            }
            if !unique.insert(*group) {
                return Err(DocumentError::DuplicateGroupMembership(*group));
            }
        }
        if before == after {
            return Ok(false);
        }
        apply_memberships_at(self, index, &before, &after)?;
        self.record_edit(
            "Set object groups",
            Edit::ObjectGroupsChanged { id, before, after },
        );
        Ok(true)
    }

    pub fn add_group(
        &mut self,
        name: Option<String>,
        members: impl IntoIterator<Item = ObjectId>,
    ) -> Result<GroupId, DocumentError> {
        let indices = self.resolve_object_indices(members)?;
        if indices.is_empty() {
            return Err(DocumentError::EmptyGroup);
        }
        self.validate_memberships_at_indices(&indices)?;
        self.group_transaction("Add group", |document| {
            let id = document.add_empty_group(name)?;
            document.append_group_at_indices(id, &indices)?;
            Ok(id)
        })
    }

    /// Clears all memberships of the requested objects, retaining definitions.
    /// Duplicate IDs coalesce; ungrouped objects are no-ops. Validate the entire
    /// batch before editing, including within a caller-owned transaction.
    pub fn clear_object_group_memberships(
        &mut self,
        objects: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        self.remove_object_group_memberships(objects, true)
    }

    /// Removes only the last ordered membership of each requested object.
    /// Retains group definitions and unrequested peers. Like complete clearing,
    /// this validates the whole batch before editing and coalesces duplicate IDs.
    pub fn pop_object_group_memberships(
        &mut self,
        objects: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        self.remove_object_group_memberships(objects, false)
    }

    fn remove_object_group_memberships(
        &mut self,
        objects: impl IntoIterator<Item = ObjectId>,
        all: bool,
    ) -> Result<usize, DocumentError> {
        let indices = self.resolve_object_indices(objects)?;
        self.validate_memberships_at_indices(&indices)?;
        let label = if all {
            "Clear object groups"
        } else {
            "Pop object groups"
        };
        self.group_transaction(label, |document| {
            let mut changed = 0;
            for index in indices {
                let memberships = &document.objects[index].group_ids;
                let retained = if all {
                    0
                } else {
                    memberships.len().saturating_sub(1)
                };
                let remaining = memberships[..retained].to_vec();
                changed += usize::from(document.set_object_group_memberships_at(index, remaining)?);
            }
            Ok(changed)
        })
    }

    /// Adds an empty group definition, including unused imported table entries.
    pub fn add_empty_group(&mut self, name: Option<String>) -> Result<GroupId, DocumentError> {
        let name = name
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty());
        if let Some(name) = &name
            && self.group_by_name(name).is_some()
        {
            return Err(DocumentError::DuplicateGroupName(name.clone()));
        }
        let id = GroupId::new();
        let index = self.groups.len();
        self.groups.push(Group {
            id,
            name,
            members: BTreeSet::new(),
        });
        self.record_edit(
            "Add group",
            Edit::GroupInserted {
                index,
                id,
                stored: None,
            },
        );
        Ok(id)
    }

    /// Appends only new memberships. Adding an existing member is a no-op,
    /// not an instruction to move that group to the top.
    pub fn add_group_members(
        &mut self,
        group_id: GroupId,
        members: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        let group = self
            .group(group_id)
            .ok_or(DocumentError::GroupNotFound(group_id))?;
        let indices = self.resolve_object_indices(members)?;
        self.validate_memberships_at_indices(&indices)?;
        for &index in &indices {
            let object = &self.objects[index];
            if object.group_ids.contains(&group_id) != group.members.contains(&object.id) {
                return Err(DocumentError::HistoryInvariant(
                    "group member index does not match",
                ));
            }
        }
        let additions = indices
            .into_iter()
            .filter(|index| !group.members.contains(&self.objects[*index].id))
            .collect::<Vec<_>>();
        self.group_transaction("Add group members", |document| {
            document.append_group_at_indices(group_id, &additions)?;
            Ok(additions.len())
        })
    }

    // Check even memberships unrelated to the requested group: a later
    // transition must not fail after earlier objects have already changed.
    pub(super) fn validate_memberships_at_indices(
        &self,
        indices: &[usize],
    ) -> Result<(), DocumentError> {
        for &index in indices {
            let memberships = &self.objects[index].group_ids;
            membership_changes(self, index, memberships, memberships)?;
        }
        Ok(())
    }

    // Call within a group transaction after resolving all objects. Membership
    // edits never reorder the object table, so each index stays valid.
    fn append_group_at_indices(
        &mut self,
        group: GroupId,
        indices: &[usize],
    ) -> Result<(), DocumentError> {
        for &index in indices {
            let mut memberships = self.objects[index].group_ids.clone();
            memberships.push(group);
            self.set_object_group_memberships_at(index, memberships)?;
        }
        Ok(())
    }

    /// Removes a definition and all memberships, retaining each surviving
    /// membership's position. Undo restores the exact original order.
    pub fn remove_group(&mut self, id: GroupId) -> Result<usize, DocumentError> {
        let index = self
            .groups
            .iter()
            .position(|g| g.id == id)
            .ok_or(DocumentError::GroupNotFound(id))?;
        let members = &self.groups[index].members;
        let indices = self.resolve_object_indices(members.iter().copied())?;
        // Validate both directions before removing a definition: otherwise an
        // absent reverse entry could leave dangling memberships, and a missing
        // object used to panic after the transaction had already started.
        for object in &self.objects {
            if object.group_ids.contains(&id) != members.contains(&object.id) {
                return Err(DocumentError::HistoryInvariant(
                    "group member index does not match",
                ));
            }
        }
        self.validate_memberships_at_indices(&indices)?;
        self.group_transaction("Remove group", |document| {
            for &object_index in &indices {
                let remaining = document.objects[object_index]
                    .group_ids
                    .iter()
                    .copied()
                    .filter(|group| *group != id)
                    .collect::<Vec<_>>();
                document.set_object_group_memberships_at(object_index, remaining)?;
            }
            let group = document.groups.remove(index);
            debug_assert!(group.members.is_empty());
            document.record_edit(
                "Remove group",
                Edit::GroupRemoved {
                    index,
                    id,
                    stored: Some(group),
                },
            );
            Ok(indices.len())
        })
    }

    /// Returns the first unused live automatic-style group name, case-sensitive.
    /// Deleted-name reservation across document history is not modeled here.
    pub fn next_unused_group_name(&self) -> String {
        GroupNames::default().next(self)
    }

    /// Recreates touched definitions on first use, walking sources in document
    /// order and memberships in each source's order. Call in the copy transaction
    /// with validated source/destination indices in source-table order. Destinations
    /// must be freshly inserted, ungrouped objects; neither table is reordered.
    pub(super) fn copy_group_memberships(
        &mut self,
        copies: &[(usize, usize)],
        assign_memberships: bool,
        names: &mut GroupNames,
    ) -> Result<(), DocumentError> {
        debug_assert!(copies.windows(2).all(|pair| pair[0].0 < pair[1].0));
        let mut mapped = BTreeMap::new();
        for &(source_index, copy_index) in copies {
            debug_assert!(source_index < copy_index);
            debug_assert!(self.objects[copy_index].group_ids.is_empty());
            let groups = self.objects[source_index].group_ids.clone();
            for group in &groups {
                if !mapped.contains_key(group) {
                    let name = names.next(self);
                    mapped.insert(*group, self.add_empty_group(Some(name))?);
                }
            }
            // These are freshly inserted, ungrouped copies. Empty source
            // memberships need no transition or per-copy object-table search.
            if assign_memberships && !groups.is_empty() {
                self.set_object_group_memberships_at(
                    copy_index,
                    groups.iter().map(|id| mapped[id]),
                )?;
            }
        }
        Ok(())
    }

    fn group_transaction<T>(
        &mut self,
        label: &'static str,
        run: impl FnOnce(&mut Self) -> Result<T, DocumentError>,
    ) -> Result<T, DocumentError> {
        let owns = self.history.active.is_none();
        if owns {
            self.begin_transaction(label)?;
        }
        let result = run(self);
        if owns {
            if result.is_ok() {
                self.commit_transaction()?;
            } else {
                self.rollback_transaction()?;
            }
        }
        result
    }
}

/// One checked transition for ordinary edits and history replay. Geometry is
/// never copied just to change groups. Validate both sides before any mutation.
pub(super) fn apply_memberships(
    document: &mut Document,
    id: ObjectId,
    before: &[GroupId],
    after: &[GroupId],
) -> Result<(), DocumentError> {
    let object_index =
        document
            .objects
            .iter()
            .position(|o| o.id == id)
            .ok_or(DocumentError::HistoryInvariant(
                "membership object is missing",
            ))?;
    apply_memberships_at(document, object_index, before, after)
}

/// Shared checked transition, with object resolution supplied by the caller.
fn apply_memberships_at(
    document: &mut Document,
    object_index: usize,
    before: &[GroupId],
    after: &[GroupId],
) -> Result<(), DocumentError> {
    let changes = membership_changes(document, object_index, before, after)?;
    let id = document.objects[object_index].id;
    for (index, present) in changes {
        if present {
            document.groups[index].members.insert(id);
        } else {
            document.groups[index].members.remove(&id);
        }
    }
    document.objects[object_index].group_ids = after.to_vec();
    Ok(())
}

fn membership_changes(
    document: &Document,
    object_index: usize,
    before: &[GroupId],
    after: &[GroupId],
) -> Result<Vec<(usize, bool)>, DocumentError> {
    let id = document.objects[object_index].id;
    if document.objects[object_index].group_ids != before {
        return Err(DocumentError::HistoryInvariant(
            "ordered object memberships do not match",
        ));
    }
    let before_set = before.iter().copied().collect::<BTreeSet<_>>();
    let after_set = after.iter().copied().collect::<BTreeSet<_>>();
    if before_set.len() != before.len() || after_set.len() != after.len() {
        return Err(DocumentError::HistoryInvariant(
            "duplicate ordered membership",
        ));
    }
    let mut changes = Vec::new();
    for group in before_set.union(&after_set) {
        let index = document.groups.iter().position(|g| g.id == *group).ok_or(
            DocumentError::HistoryInvariant("membership group is missing"),
        )?;
        if document.groups[index].members.contains(&id) != before_set.contains(group) {
            return Err(DocumentError::HistoryInvariant(
                "group member index does not match",
            ));
        }
        changes.push((index, after_set.contains(group)));
    }
    Ok(changes)
}
