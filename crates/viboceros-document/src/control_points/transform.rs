//! Validate and stage complete mixed object/grip edits before mutation.
use super::*;
#[derive(Clone, Copy, PartialEq)]
enum GripEditKind {
    Affine(bool),
    Morph,
}

impl Document {
    pub fn transform_objects_and_grips(
        &mut self,
        objects: impl IntoIterator<Item = ObjectId>,
        grips: impl IntoIterator<Item = ControlPointId>,
        transform: AffineTransform3,
        copy: bool,
        group_policy: CopyGroupPolicy,
    ) -> Result<(Vec<ObjectId>, Vec<ObjectId>), DocumentError> {
        self.map_objects_and_grips(
            objects,
            grips,
            copy,
            group_policy,
            GripEditKind::Affine(transform == AffineTransform3::identity()),
            |_, geometry, selected, mode, tolerance| {
                if let Some(selected) = selected {
                    geometry.with_transformed_grips_in_mode(selected, transform, tolerance, mode)
                } else {
                    geometry.transformed_for_edit(transform, tolerance)
                }
            },
        )
    }

    /// Stage complete mixed object/grip morphs before editing any owner.
    /// Grips take precedence over a simultaneously selected parent object.
    /// Whole copies use `group_policy`; grip copies join their source groups
    /// and enable the source grip mode without selecting the copied grips.
    pub fn morph_objects_and_grips(
        &mut self,
        objects: impl IntoIterator<Item = ObjectId>,
        grips: impl IntoIterator<Item = ControlPointId>,
        morph: &(impl viboceros_geometry::PointMorph + ?Sized),
        copy: bool,
        group_policy: CopyGroupPolicy,
    ) -> Result<(Vec<ObjectId>, Vec<ObjectId>), DocumentError> {
        self.map_objects_and_grips(
            objects,
            grips,
            copy,
            group_policy,
            GripEditKind::Morph,
            |_, geometry, selected, mode, tolerance| {
                if let Some(selected) = selected {
                    geometry.with_morphed_grips_in_mode(selected, morph, tolerance, mode)
                } else {
                    geometry.morphed(morph, tolerance)
                }
            },
        )
    }

    /// Combine already-staged whole-object geometry with selected grip morphs.
    /// All owners and grip reconstructions are validated before committing.
    /// Grip owners take precedence; duplicate staged IDs use the last geometry.
    /// Copies follow the ordinary morph group and grip-selection policies.
    pub fn morph_grips_with_object_geometries(
        &mut self,
        objects: impl IntoIterator<Item = (ObjectId, Geometry)>,
        grips: impl IntoIterator<Item = ControlPointId>,
        morph: &(impl viboceros_geometry::PointMorph + ?Sized),
        copy: bool,
        group_policy: CopyGroupPolicy,
    ) -> Result<(Vec<ObjectId>, Vec<ObjectId>), DocumentError> {
        let mut staged = objects.into_iter().collect::<BTreeMap<_, _>>();
        let ids = staged.keys().copied().collect::<Vec<_>>();
        self.map_objects_and_grips(
            ids,
            grips,
            copy,
            group_policy,
            GripEditKind::Morph,
            |id, geometry, selected, mode, tolerance| {
                if let Some(selected) = selected {
                    geometry.with_morphed_grips_in_mode(selected, morph, tolerance, mode)
                } else {
                    Ok(staged
                        .remove(&id)
                        .expect("ordinary owner has staged geometry"))
                }
            },
        )
    }

    fn map_objects_and_grips(
        &mut self,
        objects: impl IntoIterator<Item = ObjectId>,
        grips: impl IntoIterator<Item = ControlPointId>,
        copy: bool,
        group_policy: CopyGroupPolicy,
        kind: GripEditKind,
        mut map: impl FnMut(
            ObjectId,
            &Geometry,
            Option<&BTreeSet<usize>>,
            GripMode,
            Tolerance,
        ) -> Result<Geometry, GeometryError>,
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
        if !copy && kind == GripEditKind::Affine(true) {
            return Ok((owners, Vec::new()));
        }
        let staged = order
            .into_iter()
            .map(|index| {
                let source = &self.objects[index];
                let geometry = map(
                    source.id,
                    &source.geometry,
                    picks.get(&source.id),
                    self.control_points
                        .get(&source.id)
                        .map_or(GripMode::Control, |state| state.mode),
                    self.tolerance,
                )?;
                Ok((index, geometry))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Transform grips")?;
        }
        let result = (|| {
            if copy {
                let copies = if kind == GripEditKind::Morph {
                    let (grip_copies, object_copies): (Vec<_>, Vec<_>) = staged
                        .into_iter()
                        .partition(|(i, _)| picks.contains_key(&self.objects[*i].id));
                    let mut copies = self.copy_object_geometries_with_groups(
                        object_copies
                            .into_iter()
                            .map(|(i, g)| (self.objects[i].id, g))
                            .collect::<Vec<_>>(),
                        group_policy,
                    )?;
                    copies.extend(
                        self.copy_object_geometries_in_source_groups(
                            grip_copies
                                .into_iter()
                                .map(|(i, g)| (self.objects[i].id, g))
                                .collect::<Vec<_>>(),
                        )?,
                    );
                    copies
                } else {
                    self.copy_object_geometries_with_groups(
                        staged
                            .into_iter()
                            .map(|(index, g)| (self.objects[index].id, g))
                            .collect::<Vec<_>>(),
                        group_policy,
                    )?
                };
                for (&source, &copy) in owners.iter().zip(&copies) {
                    if let Some(indices) = picks.get(&source) {
                        self.enable_grips([copy], self.control_points[&source].mode)?;
                        let count = self.control_points[&copy].points.len();
                        if kind != GripEditKind::Morph {
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
