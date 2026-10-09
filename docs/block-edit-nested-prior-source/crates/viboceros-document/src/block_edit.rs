//! Isolated in-place editing of embedded block prototypes.
use super::*;
use viboceros_geometry::{Point3, Vector3};

#[derive(Clone, Debug)]
pub(super) struct BlockEditSession {
    baseline: Box<Document>,
    pub(super) original_ids: BTreeSet<ObjectId>,
    source_candidates: BTreeSet<ObjectId>,
    definition: BlockDefinitionId,
    target: ObjectId,
    placement: AffineTransform3,
    exposed_layers: Vec<Layer>,
    pub(super) settings: BlockEditSettings,
}

#[derive(Clone, Debug)]
pub(super) struct BlockEditSettings {
    pub(super) released: BTreeSet<ObjectId>,
    base_point: Point3,
    local_base: Point3,
}

impl BlockEditSession {
    pub(super) fn protects(&self, id: ObjectId) -> bool {
        self.original_ids.contains(&id) || self.settings.released.contains(&id)
    }
}

impl Document {
    pub fn is_block_edit_protected(&self, id: ObjectId) -> bool {
        self.block_edit
            .as_ref()
            .is_some_and(|edit| edit.protects(id))
    }
    pub fn is_block_editing(&self) -> bool {
        self.block_edit.is_some()
    }
    pub fn block_edit_target(&self) -> Option<ObjectId> {
        self.block_edit.as_ref().map(|e| e.target)
    }
    pub fn block_edit_base_point(&self) -> Option<Point3> {
        self.block_edit.as_ref().map(|e| e.settings.base_point)
    }
    pub fn block_edit_add_candidates(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.block_edit_add_candidate_objects().map(|o| o.id)
    }
    pub fn is_block_edit_add_candidate(&self, id: ObjectId) -> bool {
        self.block_edit.as_ref().is_some_and(|edit| {
            if id == edit.target {
                return false;
            }
            if edit.original_ids.contains(&id) {
                edit.source_candidates.contains(&id)
            } else if edit.settings.released.contains(&id) {
                self.object(id).is_some_and(|o| {
                    o.attributes.visible
                        && !o.attributes.locked
                        && self
                            .layer(o.attributes.layer_id)
                            .is_some_and(|l| l.visible && !l.locked)
                })
            } else {
                false
            }
        })
    }
    pub fn block_edit_add_candidate_objects(&self) -> impl Iterator<Item = &Object> + '_ {
        self.block_edit.iter().flat_map(|edit| {
            edit.baseline
                .selectable_objects()
                .filter(|o| o.id != edit.target)
                .chain(self.objects.iter().filter(|o| {
                    edit.settings.released.contains(&o.id)
                        && o.attributes.visible
                        && !o.attributes.locked
                        && self
                            .layer(o.attributes.layer_id)
                            .is_some_and(|layer| layer.visible && !layer.locked)
                }))
        })
    }
    pub fn block_edit_objects(&self) -> Vec<ObjectId> {
        self.block_edit.as_ref().map_or_else(Vec::new, |edit| {
            self.objects
                .iter()
                .filter(|o| !edit.protects(o.id))
                .map(|o| o.id)
                .collect()
        })
    }
    pub fn open_block_edit(&mut self, target: ObjectId) -> Result<Vec<ObjectId>, DocumentError> {
        self.ensure_no_transaction()?;
        if self.block_edit.is_some() {
            return Err(DocumentError::InvalidBlockCatalog(
                "a block edit is already open",
            ));
        }
        let object = self
            .object(target)
            .ok_or(DocumentError::ObjectNotFound(target))?;
        self.ensure_object_editable(object)?;
        let Geometry::BlockInstance(instance) = object.geometry() else {
            return Err(DocumentError::NotBlockInstance(target));
        };
        let placement = instance.reference().transform();
        let rows = placement.linear_rows();
        let columns =
            std::array::from_fn::<_, 3, _>(|j| Vector3::try_from(rows.map(|r| r[j])).unwrap());
        let lengths = [
            columns[0].length()?,
            columns[1].length()?,
            columns[2].length()?,
        ];
        let unit = columns
            .map(|v| v.normalized_nonzero().map(|u| u.as_vector()))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let epsilon = f64::EPSILON.sqrt();
        if (lengths[0] - lengths[1]).abs() > epsilon * lengths[0].max(lengths[1])
            || (lengths[0] - lengths[2]).abs() > epsilon * lengths[0].max(lengths[2])
            || unit[0].dot(unit[1])?.abs() > epsilon
            || unit[0].dot(unit[2])?.abs() > epsilon
            || unit[1].dot(unit[2])?.abs() > epsilon
        {
            return Err(DocumentError::InvalidBlockCatalog(
                "in-place editing requires a uniformly scaled instance",
            ));
        }
        let definition = instance.reference().definition();
        let members = self
            .block_definition(definition)
            .unwrap()
            .members()
            .to_vec();
        let baseline = Box::new(self.clone());
        let original_ids = self.objects.iter().map(|o| o.id).collect::<BTreeSet<_>>();
        let source_candidates = self
            .selectable_objects()
            .filter(|o| o.id != target)
            .map(|o| o.id)
            .collect();
        let mut working = self.clone();
        working.clear_selection();
        working.control_points.clear();
        for object in &mut working.objects {
            object.attributes.locked = true;
            if object.id == target {
                object.attributes.visible = false;
            }
        }
        let mut exposed = Vec::new();
        for member in members {
            let layer = working
                .layers
                .iter_mut()
                .find(|l| l.id == member.attributes().layer_id())
                .ok_or(DocumentError::LayerNotFound(member.attributes().layer_id()))?;
            layer.visible = true;
            layer.locked = false;
            let geometry = match member.content() {
                BlockContent::Geometry(g) => {
                    if placement == AffineTransform3::identity() {
                        (**g).clone()
                    } else {
                        g.transformed(
                            placement,
                            blocks::transformation_tolerance(g, self.tolerance, 1.)?,
                        )?
                    }
                }
                BlockContent::Reference(r) => working.block_instance_geometry(
                    BlockReference::try_new(r.definition(), r.transform().then(placement)?)?,
                )?,
            };
            let id = working.add_geometry_with_metadata(
                geometry,
                member.attributes().clone().with_file_state(true, false),
                member.geometry_user_text().clone(),
            )?;
            let index = working.objects.iter().position(|o| o.id == id).unwrap();
            working.objects[index].group_ids = member.group_ids().to_vec();
            for group in &mut working.groups {
                if member.group_ids().contains(&group.id) {
                    group.members.insert(id);
                }
            }
            exposed.push(id);
        }
        working.history = History::default();
        working.block_edit = Some(Box::new(BlockEditSession {
            baseline,
            original_ids,
            source_candidates,
            definition,
            target,
            placement,
            exposed_layers: working.layers.clone(),
            settings: BlockEditSettings {
                released: BTreeSet::new(),
                base_point: placement.transform_point(Point3::try_new(0., 0., 0.)?)?,
                local_base: Point3::try_new(0., 0., 0.)?,
            },
        }));
        *self = working;
        Ok(exposed)
    }
    pub fn discard_block_edit(&mut self) -> Result<(), DocumentError> {
        self.ensure_no_transaction()?;
        let edit = self
            .block_edit
            .take()
            .ok_or(DocumentError::InvalidBlockCatalog("no block edit is open"))?;
        *self = *edit.baseline;
        Ok(())
    }
    /// Copies external model objects into the active edit; originals survive.
    pub fn add_objects_to_block_edit(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        self.ensure_no_transaction()?;
        let edit = self
            .block_edit
            .as_ref()
            .ok_or(DocumentError::InvalidBlockCatalog("no block edit is open"))?;
        let indices = self.resolve_object_indices(ids)?;
        let mut copies = Vec::new();
        for index in indices {
            let object = &self.objects[index];
            if !edit.protects(object.id) || object.id == edit.target {
                return Err(DocumentError::InvalidBlockCatalog(
                    "AddObject requires objects outside the edited definition",
                ));
            }
            let attributes = if let Some(original) = edit.baseline.object(object.id) {
                edit.baseline.ensure_object_editable(original)?;
                original.attributes.clone()
            } else {
                object.attributes.clone()
            };
            copies.push((
                (*object.geometry).clone(),
                attributes.with_file_state(true, false),
                if matches!(object.geometry(), Geometry::BlockInstance(_)) {
                    BTreeMap::new()
                } else {
                    object.geometry_user_text.clone()
                },
            ));
        }
        if copies.is_empty() {
            return Ok(Vec::new());
        }
        self.begin_transaction("BlockEdit AddObject")?;
        let result = copies
            .into_iter()
            .map(|(geometry, attributes, text)| {
                self.add_geometry_with_metadata(geometry, attributes, text)
            })
            .collect::<Result<Vec<_>, _>>();
        match result {
            Ok(ids) => {
                self.commit_transaction()?;
                Ok(ids)
            }
            Err(error) => {
                self.rollback_transaction()?;
                Err(error)
            }
        }
    }
    /// Releases members to the model on SaveAndClose; discard restores them.
    pub fn remove_objects_from_block_edit(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        self.ensure_no_transaction()?;
        let edit = self
            .block_edit
            .as_ref()
            .ok_or(DocumentError::InvalidBlockCatalog("no block edit is open"))?;
        let indices = self.resolve_object_indices(ids)?;
        let ids = indices
            .into_iter()
            .map(|i| self.objects[i].id)
            .collect::<BTreeSet<_>>();
        for id in &ids {
            if edit.protects(*id) {
                return Err(DocumentError::ObjectLocked(*id));
            }
            self.ensure_object_editable(self.object(*id).unwrap())?;
        }
        if ids.is_empty() {
            return Ok(0);
        }
        self.begin_transaction("BlockEdit RemoveObject")?;
        let edit = self.block_edit.as_mut().unwrap();
        let stored = edit.settings.clone();
        edit.settings.released.extend(&ids);
        let selected = self
            .selection_order
            .iter()
            .filter(|id| ids.contains(id))
            .copied()
            .collect();
        self.record_edit(
            "BlockEdit RemoveObject",
            Edit::BlockEditSettings { stored, selected },
        );
        let selected = self.selection.difference(&ids).copied().collect();
        self.update_selection(selected);
        self.commit_transaction()?;
        Ok(ids.len())
    }
    /// Sets the world-space base point; acceptance shifts the shared definition.
    pub fn set_block_edit_base_point(&mut self, point: Point3) -> Result<bool, DocumentError> {
        self.ensure_no_transaction()?;
        let edit = self
            .block_edit
            .as_ref()
            .ok_or(DocumentError::InvalidBlockCatalog("no block edit is open"))?;
        let local_base = edit.placement.try_inverse()?.transform_point(point)?;
        if edit.settings.base_point == point {
            return Ok(false);
        }
        self.begin_transaction("BlockEdit SetBasePoint")?;
        let edit = self.block_edit.as_mut().unwrap();
        let stored = edit.settings.clone();
        edit.settings.base_point = point;
        edit.settings.local_base = local_base;
        self.record_edit(
            "BlockEdit SetBasePoint",
            Edit::BlockEditSettings {
                stored,
                selected: Vec::new(),
            },
        );
        self.commit_transaction()?;
        Ok(true)
    }
    pub(super) fn swap_block_edit_settings(
        &mut self,
        stored: &mut BlockEditSettings,
        selected: &mut Vec<ObjectId>,
    ) -> Result<(), DocumentError> {
        let edit = self
            .block_edit
            .as_mut()
            .ok_or(DocumentError::HistoryInvariant(
                "block-edit history requires an active workspace",
            ))?;
        let changed = edit
            .settings
            .released
            .symmetric_difference(&stored.released)
            .copied()
            .collect::<BTreeSet<_>>();
        std::mem::swap(&mut edit.settings, stored);
        let current = self
            .selection_order
            .iter()
            .filter(|id| changed.contains(id))
            .copied()
            .collect();
        let next = self
            .selection
            .difference(&changed)
            .copied()
            .chain(selected.iter().copied())
            .collect();
        self.update_selection(next);
        self.selection_order.retain(|id| !changed.contains(id));
        self.selection_order.extend(selected.iter().copied());
        *selected = current;
        Ok(())
    }
    pub fn save_block_edit(&mut self) -> Result<usize, DocumentError> {
        self.ensure_no_transaction()?;
        let edit = self
            .block_edit
            .as_ref()
            .ok_or(DocumentError::InvalidBlockCatalog("no block edit is open"))?;
        if self.units != edit.baseline.units {
            return Err(DocumentError::InvalidBlockCatalog(
                "finish the block edit before changing document units",
            ));
        }
        let placement_inverse = edit.placement.try_inverse()?;
        let local_base = edit.settings.local_base;
        let inverse = placement_inverse.then(AffineTransform3::from_translation(
            Vector3::try_new(-local_base.x(), -local_base.y(), -local_base.z())?,
        ))?;
        let members = self
            .objects
            .iter()
            .rev()
            .filter(|object| !edit.protects(object.id))
            .map(|object| {
                let content = match object.geometry() {
                    Geometry::BlockInstance(i) => BlockContent::Reference(BlockReference::try_new(
                        i.reference().definition(),
                        i.reference().transform().then(inverse)?,
                    )?),
                    g => BlockContent::Geometry(if inverse == AffineTransform3::identity() {
                        object.geometry.clone()
                    } else {
                        g.transformed(
                            inverse,
                            blocks::transformation_tolerance(g, self.tolerance, 1.)?,
                        )?
                        .into()
                    }),
                };
                BlockMember::captured(content, object).try_with_group_ids(Vec::new())
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        let count = members.len();
        let mut accepted = (*edit.baseline).clone();
        accepted.layers = self.layers.clone();
        for layer in &mut accepted.layers {
            if let (Some(original), Some(exposed)) = (
                edit.baseline.layer(layer.id),
                edit.exposed_layers.iter().find(|l| l.id == layer.id),
            ) {
                if layer.visible == exposed.visible {
                    layer.visible = original.visible;
                }
                if layer.locked == exposed.locked {
                    layer.locked = original.locked;
                }
            }
        }
        accepted.current_layer = self.current_layer;
        accepted.next_layer_number = self.next_layer_number;
        for object in &self.objects {
            if edit.settings.released.contains(&object.id) {
                accepted.objects.push(object.clone());
            }
        }
        accepted.groups = self.groups.clone();
        for group in &mut accepted.groups {
            group.members.retain(|id| edit.protects(*id));
        }
        accepted.block_definitions = self.block_definitions.clone();
        let index = accepted
            .block_definitions
            .iter()
            .position(|d| d.id() == edit.definition)
            .ok_or(DocumentError::BlockDefinitionNotFound(edit.definition))?;
        accepted.block_definitions[index] = accepted.block_definitions[index]
            .clone()
            .with_members(members);
        blocks::validate_catalog(&accepted, &accepted.block_definitions)?;
        let staged =
            block_instances::stage_catalog_instances(&accepted, &accepted.block_definitions)?;
        for (index, geometry) in staged {
            accepted.objects[index].geometry = geometry;
        }
        let mut stored = (*edit.baseline).clone();
        stored.history = History::default();
        stored.block_edit = None;
        accepted.begin_transaction("BlockEdit")?;
        accepted.record_edit(
            "BlockEdit",
            Edit::BlockEditModel {
                stored: Box::new(stored),
            },
        );
        accepted.commit_transaction()?;
        *self = accepted;
        Ok(count)
    }
    pub(super) fn swap_block_edit_model(&mut self, stored: &mut Document) {
        macro_rules! swap {($($field:ident),*)=>{$(std::mem::swap(&mut self.$field,&mut stored.$field);)*};}
        swap!(
            tolerance,
            units,
            layers,
            next_layer_number,
            current_layer,
            objects,
            groups,
            block_definitions,
            selection,
            selection_order,
            control_points,
            previous_selection,
            previous_selection_order,
            last_changed_objects
        );
    }
}

#[cfg(test)]
mod tests;
