//! Add model objects to an embedded definition in a selected root's frame.
use super::*;
#[path = "../../../third_party/opennurbs-rust/circle_transform.rs"]
mod conic_compatibility;

impl Document {
    /// Append editable model objects in the target instance's local frame.
    /// All uses of the definition refresh atomically. Added objects are consumed,
    /// raw attributes/text survive, and target prototype groups are cleared.
    pub fn add_objects_to_block(
        &mut self,
        target: ObjectId,
        sources: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        let object = self
            .object(target)
            .ok_or(DocumentError::ObjectNotFound(target))?;
        self.ensure_object_editable(object)?;
        let Geometry::BlockInstance(instance) = object.geometry() else {
            return Err(DocumentError::NotBlockInstance(target));
        };
        let definition = instance.reference().definition();
        let local = instance.reference().transform().try_inverse()?;
        let indices = self.resolve_object_indices(sources)?;
        if indices.is_empty() {
            return Err(DocumentError::EmptyBlock);
        }
        if indices.iter().any(|&i| self.objects[i].id() == target) {
            return Err(DocumentError::InvalidBlockCatalog(
                "a block cannot be added to itself",
            ));
        }
        self.validate_memberships_at_indices(&indices)?;
        let mut members = self
            .block_definition(definition)
            .unwrap()
            .members()
            .iter()
            .cloned()
            .map(|m| m.try_with_group_ids(Vec::new()))
            .collect::<Result<Vec<_>, _>>()?;
        for &index in &indices {
            let object = &self.objects[index];
            self.ensure_object_editable(object)?;
            let content = match object.geometry() {
                Geometry::BlockInstance(instance) => {
                    BlockContent::Reference(BlockReference::try_new(
                        instance.reference().definition(),
                        instance.reference().transform().then(local)?,
                    )?)
                }
                _ if local == AffineTransform3::identity() => {
                    BlockContent::Geometry(object.geometry.clone())
                }
                Geometry::Circle(circle) => BlockContent::Geometry(
                    Geometry::Circle(conic_compatibility::circle(*circle, local)?).into(),
                ),
                Geometry::Arc(arc) => BlockContent::Geometry(
                    Geometry::Arc(conic_compatibility::arc(*arc, local)?).into(),
                ),
                geometry => {
                    let tolerance = blocks::transformation_tolerance(geometry, self.tolerance, 1.)?;
                    BlockContent::Geometry(geometry.transformed(local, tolerance)?.into())
                }
            };
            members.push(BlockMember::captured(content, object).try_with_group_ids(Vec::new())?);
        }
        let mut catalog = self.block_definitions.clone();
        let index = catalog.iter().position(|d| d.id() == definition).unwrap();
        catalog[index] = catalog[index].clone().with_members(members);
        blocks::validate_catalog(self, &catalog)?;
        block_instances::stage_catalog_instances(self, &catalog)?;
        let ids = indices
            .iter()
            .map(|&i| self.objects[i].id())
            .collect::<Vec<_>>();
        let count = ids.len();
        let owns = self.history.active.is_none();
        if owns {
            self.begin_transaction("AddObjectsToBlock")?;
        }
        let result = (|| {
            self.set_block_definitions(catalog)?;
            self.delete_objects(ids)?;
            Ok(count)
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
