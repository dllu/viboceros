//! Validate and stage complete mixed object/grip edits before mutation.
use super::*;

impl Document {
    pub fn transform_objects_and_grips(
        &mut self,
        objects: impl IntoIterator<Item = ObjectId>,
        grips: impl IntoIterator<Item = ControlPointId>,
        transform: AffineTransform3,
        copy: bool,
        group_policy: CopyGroupPolicy,
    ) -> Result<(Vec<ObjectId>, Vec<ObjectId>), DocumentError> {
        let objects = objects.into_iter().collect::<BTreeSet<_>>();
        let mut picks: BTreeMap<ObjectId, BTreeSet<usize>> = BTreeMap::new();
        for grip in grips {
            if self
                .control_point_locations(grip.object)
                .is_none_or(|p| grip.index >= p.len())
            {
                return Err(DocumentError::InvalidControlPointSelection {
                    object: grip.object,
                    index: grip.index,
                });
            }
            picks.entry(grip.object).or_default().insert(grip.index);
        }
        let indices =
            self.resolve_object_indices(objects.iter().copied().chain(picks.keys().copied()))?;
        for &index in &indices {
            self.ensure_object_editable(&self.objects[index])?;
        }
        // Native grips take precedence over selected parents. Ordinary object
        // replacements precede grip-owner replacements in the renewed table.
        let order = indices
            .iter()
            .copied()
            .filter(|i| !picks.contains_key(&self.objects[*i].id))
            .chain(
                indices
                    .iter()
                    .copied()
                    .filter(|i| picks.contains_key(&self.objects[*i].id)),
            )
            .collect::<Vec<_>>();
        let owners = order
            .iter()
            .map(|&i| self.objects[i].id)
            .collect::<Vec<_>>();
        if !copy && transform == AffineTransform3::identity() {
            return Ok((owners, Vec::new()));
        }
        let staged = order
            .into_iter()
            .map(|index| {
                let source = &self.objects[index];
                let geometry = if let Some(selected) = picks.get(&source.id) {
                    source
                        .geometry
                        .with_transformed_grips(selected, transform, self.tolerance)?
                } else {
                    source
                        .geometry
                        .transformed_for_edit(transform, self.tolerance)?
                };
                Ok((index, geometry))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Transform grips")?;
        }
        let result =
            (|| {
                if copy {
                    let copies = self.copy_object_geometries_with_groups(
                        staged
                            .into_iter()
                            .map(|(index, geometry)| (self.objects[index].id, geometry))
                            .collect::<Vec<_>>(),
                        group_policy,
                    )?;
                    for (&source, &copy) in owners.iter().zip(&copies) {
                        if let Some(indices) = picks.get(&source) {
                            self.enable_control_points([copy])?;
                            let count = self.control_points[&copy].points.len();
                            self.select_control_points(
                                indices.iter().copied().filter(|&index| index < count).map(
                                    |index| ControlPointId {
                                        object: copy,
                                        index,
                                    },
                                ),
                                SelectionMode::Add,
                            )?;
                        }
                    }
                    Ok((owners, copies))
                } else {
                    let before = picks
                        .keys()
                        .map(|id| (*id, self.control_points[id].clone()))
                        .collect::<Vec<_>>();
                    self.commit_object_geometries(
                        staged,
                        "Transform grips",
                        "Transform grip owner",
                        ReplacementHistory::EveryReplacement,
                        true,
                    )?;
                    for (id, before) in before {
                        let current = self.control_points.get_mut(&id).unwrap();
                        // Closing an open endpoint can remove its duplicate seam
                        // grip. Native clears that vanished index rather than
                        // selecting the surviving seam grip at index zero.
                        current.selected = if before
                            .selected
                            .last()
                            .is_some_and(|&index| index >= current.points.len())
                        {
                            Arc::new(
                                before
                                    .selected
                                    .iter()
                                    .copied()
                                    .filter(|&index| index < current.points.len())
                                    .collect(),
                            )
                        } else {
                            before.selected.clone()
                        };
                        self.record_edit(
                            "Transform grips",
                            Edit::ControlPointsChanged {
                                id,
                                stored: Some(before),
                            },
                        );
                    }
                    self.move_objects_to_end_in_order(owners.iter().copied())?;
                    Ok((owners, Vec::new()))
                }
            })();
        if owns_transaction {
            match &result {
                Ok(_) => {
                    self.commit_transaction()?;
                }
                Err(_) => {
                    self.rollback_transaction()?;
                }
            }
        }
        result
    }
}
