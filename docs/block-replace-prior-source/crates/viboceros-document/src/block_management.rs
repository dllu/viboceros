//! Definition management and model-use counts without expanding geometry.
use super::*;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockDefinitionInfo {
    pub id: BlockDefinitionId,
    pub name: String,
    pub object_count: usize,
    pub top_level_instances: u64,
    pub nested_instances: u64,
    pub definition_references: usize,
}

impl BlockDefinitionInfo {
    pub fn total_instances(&self) -> u64 {
        self.top_level_instances + self.nested_instances
    }
}

impl Document {
    /// Duplicate one embedded definition, retaining nested references and
    /// immutable member geometry. No instance is added or rebound.
    pub fn duplicate_block_definition(
        &mut self,
        id: BlockDefinitionId,
        name: impl Into<String>,
    ) -> Result<BlockDefinitionId, DocumentError> {
        let original = self
            .block_definition(id)
            .ok_or(DocumentError::BlockDefinitionNotFound(id))?;
        let members = original
            .members()
            .iter()
            .cloned()
            .map(|m| m.try_with_group_ids(Vec::new()))
            .collect::<Result<Vec<_>, _>>()?;
        let copy = BlockDefinition::new(name, members);
        let result = copy.id();
        let mut catalog = self.block_definitions.clone();
        catalog.push(copy);
        self.set_block_definitions(catalog)?;
        Ok(result)
    }

    /// Rebind editable roots of one definition to one new duplicate. Geometry
    /// placement, object IDs, attributes, text and model groups are retained.
    pub fn make_block_instances_unique(
        &mut self,
        name: impl Into<String>,
        sources: impl IntoIterator<Item = ObjectId>,
    ) -> Result<BlockDefinitionId, DocumentError> {
        let indices = self.resolve_object_indices(sources)?;
        if indices.is_empty() {
            return Err(DocumentError::InvalidBlockCatalog(
                "unique block selection is empty",
            ));
        }
        let mut definition = None;
        for &index in &indices {
            let object = &self.objects[index];
            self.ensure_object_editable(object)?;
            let Geometry::BlockInstance(instance) = object.geometry() else {
                return Err(DocumentError::NotBlockInstance(object.id()));
            };
            if definition.is_some_and(|id| id != instance.reference().definition()) {
                return Err(DocumentError::InvalidBlockCatalog(
                    "unique instances must share one definition",
                ));
            }
            definition = Some(instance.reference().definition());
        }
        let original = self.block_definition(definition.unwrap()).unwrap();
        let members = original
            .members()
            .iter()
            .cloned()
            .map(|m| m.try_with_group_ids(Vec::new()))
            .collect::<Result<Vec<_>, _>>()?;
        let copy = BlockDefinition::new(name, members);
        let id = copy.id();
        let mut catalog = self.block_definitions.clone();
        catalog.push(copy);
        blocks::validate_catalog(self, &catalog)?;
        let staged = indices
            .into_iter()
            .map(|index| {
                let Geometry::BlockInstance(instance) = self.objects[index].geometry() else {
                    unreachable!()
                };
                let reference = BlockReference::try_new(id, instance.reference().transform())?;
                let geometry = Geometry::BlockInstance(block_instances::in_catalog(
                    &catalog,
                    self.tolerance,
                    reference,
                )?);
                Ok((index, geometry))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        let owns = self.history.active.is_none();
        if owns {
            self.begin_transaction("CreateUniqueBlock")?;
        }
        let result = (|| {
            self.set_block_definitions(catalog)?;
            self.commit_object_geometries(
                staged,
                "CreateUniqueBlock",
                "Rebind block definition",
                ReplacementHistory::EveryReplacement,
                true,
            )?;
            Ok(id)
        })();
        if owns {
            if result.is_ok() {
                self.commit_transaction()?;
            } else {
                self.rollback_transaction()?;
            }
        }
        result
    }

    /// Count each placed occurrence, including repeated nested placements.
    /// Prototype reference counts separately determine whether deletion is legal.
    /// The graph pass is linear in catalog references and does not tessellate.
    pub fn block_definition_info(&self) -> Result<Vec<BlockDefinitionInfo>, DocumentError> {
        let definitions = &self.block_definitions;
        let index = definitions
            .iter()
            .enumerate()
            .map(|(i, d)| (d.id(), i))
            .collect::<BTreeMap<_, _>>();
        let mut rows = definitions
            .iter()
            .map(|d| BlockDefinitionInfo {
                id: d.id(),
                name: d.name().into(),
                object_count: d.members().len(),
                top_level_instances: 0,
                nested_instances: 0,
                definition_references: 0,
            })
            .collect::<Vec<_>>();
        let mut edges = vec![Vec::new(); definitions.len()];
        for (i, definition) in definitions.iter().enumerate() {
            for member in definition.members() {
                if let BlockContent::Reference(r) = member.content() {
                    let child = *index
                        .get(&r.definition())
                        .ok_or(DocumentError::BlockDefinitionNotFound(r.definition()))?;
                    edges[i].push(child);
                    rows[child].definition_references += 1;
                }
            }
        }
        for object in self.objects() {
            if let Geometry::BlockInstance(instance) = object.geometry() {
                let id = instance.reference().definition();
                let i = *index
                    .get(&id)
                    .ok_or(DocumentError::BlockDefinitionNotFound(id))?;
                rows[i].top_level_instances = rows[i].top_level_instances.checked_add(1).ok_or(
                    DocumentError::InvalidBlockCatalog("block use count overflow"),
                )?;
            }
        }
        let mut pending = rows
            .iter()
            .map(|r| r.definition_references)
            .collect::<Vec<_>>();
        let mut queue = pending
            .iter()
            .enumerate()
            .filter_map(|(i, n)| (*n == 0).then_some(i))
            .collect::<VecDeque<_>>();
        let mut visited = 0;
        while let Some(i) = queue.pop_front() {
            visited += 1;
            let count = rows[i]
                .top_level_instances
                .checked_add(rows[i].nested_instances)
                .ok_or(DocumentError::InvalidBlockCatalog(
                    "block use count overflow",
                ))?;
            for &child in &edges[i] {
                rows[child].nested_instances =
                    rows[child].nested_instances.checked_add(count).ok_or(
                        DocumentError::InvalidBlockCatalog("block use count overflow"),
                    )?;
                pending[child] -= 1;
                if pending[child] == 0 {
                    queue.push_back(child);
                }
            }
        }
        if visited != definitions.len() {
            return Err(DocumentError::InvalidBlockCatalog("cyclic block use graph"));
        }
        for row in &rows {
            row.top_level_instances
                .checked_add(row.nested_instances)
                .ok_or(DocumentError::InvalidBlockCatalog(
                    "block use count overflow",
                ))?;
        }
        Ok(rows)
    }

    pub fn rename_block_definition(
        &mut self,
        id: BlockDefinitionId,
        name: impl Into<String>,
    ) -> Result<bool, DocumentError> {
        let mut definitions = self.block_definitions.clone();
        let i = definitions
            .iter()
            .position(|d| d.id() == id)
            .ok_or(DocumentError::BlockDefinitionNotFound(id))?;
        definitions[i] = definitions[i].clone().with_name(name);
        self.set_block_definitions(definitions)
    }

    /// Delete a definition and all its model roots, including hidden/locked
    /// roots. Definitions nested in any other catalog entry cannot be deleted.
    pub fn delete_block_definition_and_instances(
        &mut self,
        id: BlockDefinitionId,
    ) -> Result<usize, DocumentError> {
        self.block_definition(id)
            .ok_or(DocumentError::BlockDefinitionNotFound(id))?;
        if self.block_definitions().any(|d| {
            d.members()
                .iter()
                .any(|m| matches!(m.content(),BlockContent::Reference(r) if r.definition()==id))
        }) {
            return Err(DocumentError::InvalidBlockCatalog(
                "definition is nested in another block",
            ));
        }
        let roots = self
            .objects()
            .filter_map(|o| {
                matches!(o.geometry(),Geometry::BlockInstance(i) if i.reference().definition()==id)
                    .then_some(o.id())
            })
            .collect::<Vec<_>>();
        let owns = self.history.active.is_none();
        if owns {
            self.begin_transaction("Delete block definition")?;
        }
        let result = (|| {
            let removed = self.delete_objects(roots)?;
            self.remove_block_definition(id)?;
            Ok(removed)
        })();
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

#[cfg(test)]
mod tests;
