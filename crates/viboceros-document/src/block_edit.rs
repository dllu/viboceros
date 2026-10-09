//! Isolated in-place editing of embedded block prototypes.
use super::*;
use viboceros_geometry::Vector3;

#[derive(Clone, Debug)]
pub(super) struct BlockEditSession {
    baseline: Box<Document>,
    pub(super) original_ids: BTreeSet<ObjectId>,
    definition: BlockDefinitionId,
    target: ObjectId,
    placement: AffineTransform3,
    exposed_layers: Vec<Layer>,
}

impl Document {
    pub fn is_block_editing(&self) -> bool {
        self.block_edit.is_some()
    }
    pub fn block_edit_target(&self) -> Option<ObjectId> {
        self.block_edit.as_ref().map(|e| e.target)
    }
    pub fn block_edit_objects(&self) -> Vec<ObjectId> {
        self.block_edit.as_ref().map_or_else(Vec::new, |edit| {
            self.objects
                .iter()
                .filter(|o| !edit.original_ids.contains(&o.id))
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
            definition,
            target,
            placement,
            exposed_layers: working.layers.clone(),
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
        let inverse = edit.placement.try_inverse()?;
        let ids = self.block_edit_objects();
        let members = ids
            .iter()
            .map(|id| {
                let object = self.object(*id).unwrap();
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
        accepted.groups = self.groups.clone();
        for group in &mut accepted.groups {
            group.members.retain(|id| edit.original_ids.contains(id));
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
