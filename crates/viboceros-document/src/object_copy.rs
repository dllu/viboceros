//! Geometry copies, independent instances, and group-aware copy commits.

use super::*;

#[cfg(test)]
mod tests;

impl Document {
    pub fn copy_objects_transformed(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        transform: AffineTransform3,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        self.copy_objects_with_transforms(ids, &[transform])
    }

    /// Atomically copies one source set through each transform. Source order,
    /// attributes, and overlapping group topology are preserved independently
    /// for every transformed instance.
    pub fn copy_objects_with_transforms(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        transforms: &[AffineTransform3],
    ) -> Result<Vec<ObjectId>, DocumentError> {
        self.copy_objects_with_transforms_and_groups(ids, transforms, CopyGroupPolicy::Preserve)
    }

    /// Copies an entire source set with an explicit group-membership policy.
    /// Geometry and attributes are always preserved; `Omit` does not allocate
    /// empty group records. Original groups are never changed.
    pub fn copy_objects_with_transforms_and_groups(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        transforms: &[AffineTransform3],
        group_policy: CopyGroupPolicy,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        if transforms.is_empty() {
            return Ok(Vec::new());
        }
        let sources = self.resolve_object_indices(ids)?;
        for index in &sources {
            self.ensure_object_editable(&self.objects[*index])?;
        }
        if sources.is_empty() {
            return Ok(Vec::new());
        }

        let copy_count = sources
            .len()
            .checked_mul(transforms.len())
            .ok_or(DocumentError::TooManyObjectCopies)?;
        let mut staged = Vec::new();
        staged
            .try_reserve_exact(copy_count)
            .map_err(|_| DocumentError::TooManyObjectCopies)?;
        for transform in transforms {
            for source_index in &sources {
                staged.push((
                    *source_index,
                    self.objects[*source_index]
                        .geometry
                        .transformed(*transform, self.tolerance)?,
                ));
            }
        }

        self.copy_staged_object_sets(&sources, transforms.len(), staged, group_policy)
    }

    /// Atomically copies one source set through a non-affine point morph.
    /// Attributes and overlapping group topology are preserved for the copy.
    pub fn copy_objects_morphed(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        morph: &(impl PointMorph + ?Sized),
    ) -> Result<Vec<ObjectId>, DocumentError> {
        let staged =
            self.stage_object_geometries(ids, |geometry| geometry.morphed(morph, self.tolerance))?;
        if staged.is_empty() {
            return Ok(Vec::new());
        }
        let sources = staged.iter().map(|(index, _)| *index).collect::<Vec<_>>();
        self.copy_staged_object_sets(&sources, 1, staged, CopyGroupPolicy::Preserve)
    }

    /// Atomically copies replacement geometry while preserving source
    /// attributes and appending each copy to every group containing its source.
    /// Source selection is retained and the new objects remain unselected.
    pub fn copy_object_geometries_into_source_groups(
        &mut self,
        copies: impl IntoIterator<Item = (ObjectId, Geometry)>,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        self.copy_object_geometries_with_order(copies, false)
    }

    /// Copies in caller-specified order, retaining source attributes and group
    /// memberships. Duplicate IDs use their last geometry and last position.
    pub fn copy_object_geometries_into_source_groups_in_order(
        &mut self,
        copies: impl IntoIterator<Item = (ObjectId, Geometry)>,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        self.copy_object_geometries_with_order(copies, true)
    }

    /// Copies every supplied piece in input order, including repeated source
    /// IDs. Each piece inherits its source's attributes and ordered groups.
    /// All distinct sources are validated before insertion; selected restricted
    /// group peers are editable under the ordinary source-editing policy.
    pub fn copy_object_pieces_into_source_groups(
        &mut self,
        pieces: impl IntoIterator<Item = (ObjectId, Geometry)>,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        let pieces = pieces.into_iter().collect::<Vec<_>>();
        let indices = self.resolve_object_indices(pieces.iter().map(|(id, _)| *id))?;
        for &index in &indices {
            self.ensure_object_editable(&self.objects[index])?;
        }
        self.validate_memberships_at_indices(&indices)?;
        let by_id = indices
            .into_iter()
            .map(|index| (self.objects[index].id, index))
            .collect::<BTreeMap<_, _>>();
        self.commit_source_group_copies(
            pieces
                .into_iter()
                .map(|(id, geometry)| (by_id[&id], geometry)),
        )
    }

    fn copy_object_geometries_with_order(
        &mut self,
        copies: impl IntoIterator<Item = (ObjectId, Geometry)>,
        input_order: bool,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        let mut copies = copies
            .into_iter()
            .enumerate()
            .map(|(rank, (id, geometry))| (id, (rank, geometry)))
            .collect::<BTreeMap<_, _>>();
        let indices = self.resolve_object_indices(copies.keys().copied())?;
        for &index in &indices {
            self.ensure_object_editable(&self.objects[index])?;
        }
        self.validate_memberships_at_indices(&indices)?;

        let mut staged = Vec::with_capacity(copies.len());
        for index in indices {
            let object = &self.objects[index];
            let (rank, geometry) = copies.remove(&object.id).unwrap();
            staged.push((index, rank, geometry));
        }
        if staged.is_empty() {
            return Ok(Vec::new());
        }
        if input_order {
            staged.sort_unstable_by_key(|(_, rank, _)| *rank);
        }

        self.commit_source_group_copies(
            staged
                .into_iter()
                .map(|(index, _, geometry)| (index, geometry)),
        )
    }

    // Source indices, editability, and memberships must have been validated;
    // only appends may change the object table before memberships are assigned.
    fn commit_source_group_copies(
        &mut self,
        staged: impl ExactSizeIterator<Item = (usize, Geometry)>,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        if staged.len() == 0 {
            return Ok(Vec::new());
        }
        self.objects
            .try_reserve_exact(staged.len())
            .map_err(|_| DocumentError::TooManyObjectCopies)?;
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Copy object geometries")?;
        }

        let mut copied = Vec::with_capacity(staged.len());
        for (source_index, geometry) in staged {
            let source = &self.objects[source_index];
            let attributes = source.attributes.clone();
            let copy_id = ObjectId::new();
            let index = self.objects.len();
            self.objects.push(Object {
                id: copy_id,
                geometry,
                attributes,
                isolation: ObjectIsolation::None,
                group_ids: Vec::new(),
            });
            self.record_edit(
                "Copy object geometry",
                Edit::ObjectInserted {
                    index,
                    id: copy_id,
                    stored: None,
                    selected: false,
                },
            );
            copied.push((source_index, index, copy_id));
        }

        for &(source_index, copy_index, _) in &copied {
            let memberships = self.objects[source_index].group_ids.clone();
            if !memberships.is_empty() {
                self.set_object_group_memberships_at(copy_index, memberships)?;
            }
        }

        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(copied.into_iter().map(|(_, _, copy_id)| copy_id).collect())
    }

    fn copy_staged_object_sets(
        &mut self,
        sources: &[usize],
        instance_count: usize,
        staged: Vec<(usize, Geometry)>,
        group_policy: CopyGroupPolicy,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        let copy_count = sources
            .len()
            .checked_mul(instance_count)
            .ok_or(DocumentError::TooManyObjectCopies)?;
        if staged.len() != copy_count {
            return Err(DocumentError::TooManyObjectCopies);
        }

        let group_copy_count = if group_policy != CopyGroupPolicy::Omit {
            self.validate_memberships_at_indices(sources)?;
            let group_count = sources
                .iter()
                .flat_map(|index| self.objects[*index].group_ids.iter().copied())
                .collect::<BTreeSet<_>>()
                .len();
            group_count
                .checked_mul(instance_count)
                .ok_or(DocumentError::TooManyObjectCopies)?
        } else {
            0
        };
        self.objects
            .try_reserve_exact(copy_count)
            .map_err(|_| DocumentError::TooManyObjectCopies)?;
        self.groups
            .try_reserve_exact(group_copy_count)
            .map_err(|_| DocumentError::TooManyObjectCopies)?;

        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Copy objects")?;
        }
        let mut copied_ids = Vec::with_capacity(copy_count);
        let mut staged = staged.into_iter();
        let mut group_names = groups::GroupNames::default();
        for _ in 0..instance_count {
            let mut copied_indices = Vec::new();
            for _ in sources {
                let (source_index, geometry) = staged
                    .next()
                    .expect("each transform has one staged geometry per source");
                let source = &self.objects[source_index];
                let attributes = source.attributes.clone();
                let id = ObjectId::new();
                let index = self.objects.len();
                self.objects.push(Object {
                    id,
                    geometry,
                    attributes,
                    isolation: ObjectIsolation::None,
                    group_ids: Vec::new(),
                });
                self.record_edit(
                    "Copy object",
                    Edit::ObjectInserted {
                        index,
                        id,
                        stored: None,
                        selected: false,
                    },
                );
                if group_policy != CopyGroupPolicy::Omit {
                    copied_indices.push((source_index, index));
                }
                copied_ids.push(id);
            }
            if group_policy != CopyGroupPolicy::Omit {
                self.copy_group_memberships(
                    &copied_indices,
                    group_policy == CopyGroupPolicy::Preserve,
                    &mut group_names,
                )?;
            }
        }
        self.update_selection(copied_ids.iter().copied().collect());
        self.prune_selection();
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(copied_ids)
    }
}
