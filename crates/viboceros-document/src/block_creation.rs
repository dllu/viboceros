//! Capture model objects in block-local coordinates without flattening instances.
use super::*;
use viboceros_geometry::{Point3, Vector3};

impl Document {
    /// Replaces source objects with one instance at the picked base point.
    /// Reusing a name keeps the definition ID and updates existing instances.
    /// Members retain raw attributes, geometry text and ordered group metadata.
    /// The new root uses the current layer's default attributes and is unselected.
    pub fn create_block_from_objects(
        &mut self,
        name: impl Into<String>,
        base: Point3,
        sources: impl IntoIterator<Item = ObjectId>,
    ) -> Result<(BlockDefinitionId, ObjectId), DocumentError> {
        let indices = self.resolve_object_indices(sources)?;
        if indices.is_empty() {
            return Err(DocumentError::EmptyBlock);
        }
        for &index in &indices {
            self.ensure_object_editable(&self.objects[index])?;
        }
        self.validate_memberships_at_indices(&indices)?;
        let local =
            AffineTransform3::from_translation(Vector3::try_from(base.to_array())?.scaled(-1.)?);
        let members = indices
            .iter()
            .map(|&index| {
                let object = &self.objects[index];
                let content = if let Geometry::BlockInstance(instance) = &*object.geometry {
                    BlockContent::Reference(BlockReference::try_new(
                        instance.reference().definition(),
                        instance.reference().transform().then(local)?,
                    )?)
                } else if local == AffineTransform3::identity() {
                    BlockContent::Geometry(object.geometry.clone())
                } else {
                    let tolerance = super::blocks::transformation_tolerance(
                        &object.geometry,
                        self.tolerance,
                        1.,
                    )?;
                    BlockContent::Geometry(object.geometry.transformed(local, tolerance)?.into())
                };
                Ok(BlockMember::captured(content, object))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        let name = name.into();
        let definition = if let Some(definition) = self.block_definition_by_name(&name) {
            definition.clone().with_members(members)
        } else {
            BlockDefinition::new(&name, members)
        };
        let definition_id = definition.id();
        let mut catalog = self.block_definitions.clone();
        if let Some(index) = catalog
            .iter()
            .position(|definition| definition.id() == definition_id)
        {
            catalog[index] = definition;
        } else {
            catalog.push(definition);
        }
        super::blocks::validate_catalog(self, &catalog)?;
        super::block_instances::stage_catalog_instances(self, &catalog)?;
        let placement = BlockReference::try_new(
            definition_id,
            AffineTransform3::from_translation(Vector3::try_from(base.to_array())?),
        )?;
        let geometry = Geometry::BlockInstance(super::block_instances::in_catalog(
            &catalog,
            self.tolerance,
            placement,
        )?);
        let sources = indices
            .iter()
            .map(|&index| self.objects[index].id)
            .collect::<Vec<_>>();
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction("Block")?;
        }
        let result = (|| {
            self.set_block_definitions(catalog)?;
            let instance = self.add_geometry(geometry)?;
            self.delete_objects(sources)?;
            Ok((definition_id, instance))
        })();
        if owns_transaction {
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
