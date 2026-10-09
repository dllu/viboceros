//! Atomic nested-definition context changes with isolated workspace history.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct BlockEditTreeNode {
    pub path: Vec<usize>,
    pub definition: BlockDefinitionId,
    pub name: String,
    pub placement: AffineTransform3,
    pub editable: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockEditTreeRevision {
    version: uuid::Uuid,
    pending_edits: usize,
}

impl Document {
    pub fn block_edit_tree_revision(&self) -> BlockEditTreeRevision {
        BlockEditTreeRevision {
            version: self.history.version,
            pending_edits: self.history.active.as_ref().map_or(0, |t| t.edits.len()),
        }
    }
    pub fn block_edit_definition(&self) -> Option<BlockDefinitionId> {
        self.block_edit.as_ref().map(|edit| edit.definition)
    }
    pub fn block_edit_path(&self) -> Option<&[usize]> {
        self.block_edit.as_ref().map(|edit| edit.path.as_slice())
    }
    fn block_edit_context_changed(&self) -> bool {
        let Some(edit) = &self.block_edit else {
            return false;
        };
        if edit.settings.local_base != Point3::try_new(0., 0., 0.).unwrap() {
            return true;
        }
        let mut count = 0;
        for object in self.objects.iter().filter(|o| !edit.protects(o.id)) {
            count += 1;
            let Some(initial) = edit.initial_objects.get(&object.id) else {
                return true;
            };
            if object.attributes != initial.attributes
                || object.geometry_user_text != initial.geometry_user_text
                || object.group_ids != initial.group_ids
            {
                return true;
            }
            match (object.geometry(), initial.geometry()) {
                (Geometry::BlockInstance(a), Geometry::BlockInstance(b))
                    if a.reference() == b.reference() => {}
                (a, b) if a == b => {}
                _ => return true,
            }
        }
        count != edit.initial_objects.len()
    }
    fn block_edit_effective_catalog(&self) -> Result<Vec<BlockDefinition>, DocumentError> {
        let mut catalog = self.block_definitions.clone();
        if self.block_edit_context_changed() {
            let edit = self.block_edit.as_ref().unwrap();
            let index = catalog
                .iter()
                .position(|d| d.id() == edit.definition)
                .ok_or(DocumentError::BlockDefinitionNotFound(edit.definition))?;
            catalog[index] = catalog[index]
                .clone()
                .with_members(self.capture_block_edit_members()?);
        }
        blocks::validate_catalog(self, &catalog)?;
        Ok(catalog)
    }
    pub fn block_edit_tree(&self) -> Result<Vec<BlockEditTreeNode>, DocumentError> {
        let edit = self
            .block_edit
            .as_ref()
            .ok_or(DocumentError::InvalidBlockCatalog("no block edit is open"))?;
        let catalog = self.block_edit_effective_catalog()?;
        let index = catalog
            .iter()
            .map(|d| (d.id(), d))
            .collect::<BTreeMap<_, _>>();
        let mut result = Vec::new();
        let mut pending = vec![(Vec::new(), edit.root_definition, edit.root_placement)];
        while let Some((path, id, placement)) = pending.pop() {
            if result.len() >= 10000 {
                return Err(DocumentError::InvalidBlockCatalog(
                    "block edit tree exceeded its bound",
                ));
            }
            let definition = index
                .get(&id)
                .ok_or(DocumentError::BlockDefinitionNotFound(id))?;
            result.push(BlockEditTreeNode {
                path: path.clone(),
                definition: id,
                name: definition.name().to_owned(),
                placement,
                editable: check_edit_placement(placement).is_ok(),
            });
            for (member_index, member) in definition.members().iter().enumerate().rev() {
                if let BlockContent::Reference(reference) = member.content() {
                    let mut child = path.clone();
                    child.push(member_index);
                    pending.push((
                        child,
                        reference.definition(),
                        reference.transform().then(placement)?,
                    ));
                }
            }
        }
        Ok(result)
    }
    /// Changes the active member path within the selected root instance.
    /// Preparation and graph/placement checks finish before any model change.
    pub fn switch_block_edit_context(
        &mut self,
        path: &[usize],
    ) -> Result<Vec<ObjectId>, DocumentError> {
        self.ensure_no_transaction()?;
        let edit = self
            .block_edit
            .as_ref()
            .ok_or(DocumentError::InvalidBlockCatalog("no block edit is open"))?;
        if path == edit.path {
            return Ok(self.block_edit_objects());
        }
        if path.len() > 64 {
            return Err(DocumentError::InvalidBlockCatalog(
                "block edit path exceeded its bound",
            ));
        }
        let catalog = self.block_edit_effective_catalog()?;
        let mut definition = edit.root_definition;
        let mut placement = edit.root_placement;
        for member in path {
            let parent = catalog
                .iter()
                .find(|d| d.id() == definition)
                .ok_or(DocumentError::BlockDefinitionNotFound(definition))?;
            let Some(BlockContent::Reference(reference)) =
                parent.members().get(*member).map(BlockMember::content)
            else {
                return Err(DocumentError::InvalidBlockCatalog(
                    "block edit path must follow nested references",
                ));
            };
            definition = reference.definition();
            placement = reference.transform().then(placement)?;
        }
        check_edit_placement(placement)?;
        let checkpoint = self.block_edit_checkpoint(&catalog)?;
        let mut working = self.clone_block_edit_model();
        let edit = working.block_edit.as_mut().unwrap();
        let retained = edit
            .original_ids
            .union(&edit.settings.released)
            .copied()
            .collect::<BTreeSet<_>>();
        working.objects.retain(|o| retained.contains(&o.id));
        for group in &mut working.groups {
            group.members.retain(|id| retained.contains(id));
        }
        working.clear_selection();
        working.control_points.clear();
        working.block_definitions = catalog;
        let staged =
            block_instances::stage_catalog_instances(&working, &working.block_definitions)?;
        for (index, geometry) in staged {
            working.objects[index].geometry = geometry;
        }
        let mut parent = working.block_edit.as_ref().unwrap().root_definition;
        let mut parent_placement = working.block_edit.as_ref().unwrap().root_placement;
        let mut background = BTreeSet::new();
        for chosen in path {
            let members = working.block_definition(parent).unwrap().members().to_vec();
            let BlockContent::Reference(reference) = members[*chosen].content() else {
                unreachable!()
            };
            let next = reference.definition();
            let next_placement = reference.transform().then(parent_placement)?;
            for (index, member) in members.iter().enumerate() {
                if index == *chosen {
                    continue;
                }
                for id in
                    working.expose_block_members(std::slice::from_ref(member), parent_placement)?
                {
                    working
                        .objects
                        .iter_mut()
                        .find(|o| o.id == id)
                        .unwrap()
                        .attributes
                        .locked = true;
                    background.insert(id);
                }
            }
            parent = next;
            parent_placement = next_placement;
        }
        let members = working
            .block_definition(definition)
            .unwrap()
            .members()
            .to_vec();
        let exposed = working.expose_block_members(&members, placement)?;
        let initial = exposed
            .iter()
            .map(|id| (*id, working.object(*id).unwrap().clone()))
            .collect();
        let edit = working.block_edit.as_mut().unwrap();
        edit.definition = definition;
        edit.placement = placement;
        edit.path = path.to_vec();
        edit.background_ids = background;
        edit.initial_objects = initial;
        edit.settings.base_point = placement.transform_point(Point3::try_new(0., 0., 0.)?)?;
        edit.settings.local_base = Point3::try_new(0., 0., 0.)?;
        edit.cancel_checkpoint = Arc::new(checkpoint);
        edit.exposed_layers = working.layers.clone();
        let stored = self.clone_block_edit_model();
        working.history = std::mem::take(&mut self.history);
        working.begin_transaction("BlockEdit context")?;
        working.record_edit(
            "BlockEdit context",
            Edit::BlockEditNavigation {
                stored: Box::new(stored),
            },
        );
        working.commit_transaction()?;
        *self = working;
        Ok(exposed)
    }
    pub(super) fn clone_block_edit_model(&self) -> Document {
        Document {
            block_edit: self.block_edit.clone(),
            tolerance: self.tolerance,
            units: self.units.clone(),
            layers: self.layers.clone(),
            next_layer_number: self.next_layer_number,
            current_layer: self.current_layer,
            objects: self.objects.clone(),
            groups: self.groups.clone(),
            block_definitions: self.block_definitions.clone(),
            selection: self.selection.clone(),
            selection_order: self.selection_order.clone(),
            control_points: self.control_points.clone(),
            previous_selection: self.previous_selection.clone(),
            previous_selection_order: self.previous_selection_order.clone(),
            last_changed_objects: self.last_changed_objects.clone(),
            history: History::default(),
        }
    }
    fn block_edit_checkpoint(
        &self,
        catalog: &[BlockDefinition],
    ) -> Result<Document, DocumentError> {
        let edit = self.block_edit.as_ref().unwrap();
        let mut checkpoint = (*edit.cancel_checkpoint).clone();
        checkpoint.layers = self.layers.clone();
        self.restore_block_edit_layer_flags(&mut checkpoint);
        checkpoint.current_layer = self.current_layer;
        checkpoint.next_layer_number = self.next_layer_number;
        checkpoint.block_definitions = catalog.to_vec();
        let active = checkpoint
            .block_definitions
            .iter()
            .position(|d| d.id() == edit.definition)
            .ok_or(DocumentError::BlockDefinitionNotFound(edit.definition))?;
        checkpoint.block_definitions[active] = checkpoint.block_definitions[active]
            .clone()
            .with_members(self.capture_block_edit_members()?);
        for object in &self.objects {
            if edit.settings.released.contains(&object.id) && checkpoint.object(object.id).is_none()
            {
                checkpoint.objects.push(object.clone());
            }
        }
        checkpoint.groups = self.groups.clone();
        let ids = checkpoint
            .objects
            .iter()
            .map(|o| o.id)
            .collect::<BTreeSet<_>>();
        for group in &mut checkpoint.groups {
            group.members.retain(|id| ids.contains(id));
        }
        let staged =
            block_instances::stage_catalog_instances(&checkpoint, &checkpoint.block_definitions)?;
        for (index, geometry) in staged {
            checkpoint.objects[index].geometry = geometry;
        }
        // Context acceptance stays in the one eventual BlockEdit model step.
        checkpoint.block_edit = None;
        Ok(checkpoint)
    }
}

#[cfg(test)]
mod tests;
