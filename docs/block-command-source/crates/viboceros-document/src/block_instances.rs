//! Instance objects retain a catalog reference; placed leaves are immutable caches.
use super::*;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct BlockInstance {
    reference: BlockReference,
    members: Arc<Vec<ResolvedBlockMember>>,
    bounds: BoundingBox3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockMemberDisplay {
    pub color: ColorRgb,
    pub visible: bool,
    pub locked: bool,
}

impl BlockInstance {
    pub const fn reference(&self) -> BlockReference {
        self.reference
    }
    pub fn members(&self) -> &[ResolvedBlockMember] {
        &self.members
    }
    pub const fn bounds(&self) -> BoundingBox3 {
        self.bounds
    }

    pub fn tight_bounds(&self, tolerance: Tolerance) -> Result<BoundingBox3, GeometryError> {
        let bounds = self
            .members
            .iter()
            .map(|member| member.geometry.tight_bounds(tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        BoundingBox3::from_points(
            bounds
                .into_iter()
                .flat_map(|bounds| [bounds.min(), bounds.max()]),
        )
    }

    pub(super) fn transformed(
        &self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        transform.orientation_reversing()?;
        let placement = self.reference.transform().then(transform)?;
        placement.orientation_reversing()?;
        let members = self
            .members
            .iter()
            .map(|member| {
                let tolerance =
                    super::blocks::transformation_tolerance(&member.geometry, tolerance, 1.)?;
                Ok(ResolvedBlockMember {
                    geometry: member.geometry.transformed(transform, tolerance)?.into(),
                    ..member.clone()
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok(Self {
            reference: BlockReference::try_new(self.reference.definition(), placement)
                .expect("the checked finite nonsingular placement is valid"),
            bounds: member_bounds(&members)?,
            members: Arc::new(members),
        })
    }
}

fn member_bounds(members: &[ResolvedBlockMember]) -> Result<BoundingBox3, GeometryError> {
    BoundingBox3::from_points(members.iter().flat_map(|member| {
        let bounds = member.geometry.bounds();
        [bounds.min(), bounds.max()]
    }))
}

pub(super) fn in_catalog(
    definitions: &[BlockDefinition],
    tolerance: Tolerance,
    reference: BlockReference,
) -> Result<BlockInstance, DocumentError> {
    let members = super::blocks::resolve_in_catalog(definitions, tolerance, reference)?;
    if members.is_empty() {
        return Err(super::blocks::invalid(
            "empty block instance geometry is unsupported",
        ));
    }
    Ok(BlockInstance {
        reference,
        bounds: member_bounds(&members)?,
        members: Arc::new(members),
    })
}

impl Document {
    pub(super) fn transformed_geometry_for_edit(
        &self,
        geometry: &Geometry,
        transform: AffineTransform3,
    ) -> Result<Geometry, DocumentError> {
        if let Geometry::BlockInstance(instance) = geometry {
            let placement = instance.reference().transform().then(transform)?;
            self.block_instance_geometry(BlockReference::try_new(
                instance.reference().definition(),
                placement,
            )?)
        } else {
            Ok(geometry.transformed_for_edit(transform, self.tolerance)?)
        }
    }
    /// Local hierarchical display policy: child ByParent colors inherit their
    /// parent color; visibility is intersected and locking is accumulated.
    /// Material sources use the document's existing layer-color fallback.
    pub fn block_member_display(
        &self,
        root: &ObjectAttributes,
        path: &[BlockMemberLocation],
    ) -> Result<BlockMemberDisplay, DocumentError> {
        Ok(self.block_displays_for_paths(root, std::iter::once(path))?[0])
    }

    /// Resolves all leaf display states with one definition/layer lookup index.
    pub fn block_instance_member_displays(
        &self,
        root: &ObjectAttributes,
        instance: &BlockInstance,
    ) -> Result<Vec<BlockMemberDisplay>, DocumentError> {
        self.block_displays_for_paths(
            root,
            instance
                .members()
                .iter()
                .map(|member| member.path.as_slice()),
        )
    }

    fn block_displays_for_paths<'a>(
        &self,
        root: &ObjectAttributes,
        paths: impl IntoIterator<Item = &'a [BlockMemberLocation]>,
    ) -> Result<Vec<BlockMemberDisplay>, DocumentError> {
        let definitions = self
            .block_definitions
            .iter()
            .map(|definition| (definition.id(), definition))
            .collect::<BTreeMap<_, _>>();
        let layers = self
            .layers
            .iter()
            .map(|layer| (layer.id(), layer))
            .collect::<BTreeMap<_, _>>();
        let layer = layers
            .get(&root.layer_id())
            .ok_or(DocumentError::LayerNotFound(root.layer_id()))?;
        let initial = BlockMemberDisplay {
            color: root.display_color(layer.color()),
            visible: root.is_visible() && layer.is_visible(),
            locked: root.is_locked() || layer.is_locked(),
        };
        paths
            .into_iter()
            .map(|path| {
                let mut display = initial;
                for location in path {
                    let member = definitions
                        .get(&location.definition)
                        .ok_or(DocumentError::BlockDefinitionNotFound(location.definition))?
                        .members()
                        .get(location.member_index)
                        .ok_or(super::blocks::invalid("block display member is missing"))?;
                    let attributes = member.attributes();
                    let layer = layers
                        .get(&attributes.layer_id())
                        .ok_or(DocumentError::LayerNotFound(attributes.layer_id()))?;
                    if attributes.color_source() != ObjectColorSource::Parent {
                        display.color = attributes.display_color(layer.color());
                    }
                    display.visible &= attributes.is_visible() && layer.is_visible();
                    display.locked |= attributes.is_locked();
                }
                Ok(display)
            })
            .collect()
    }
    /// Produces an instance with derived geometry from the current catalog.
    /// Admission/replacement revalidates it against the receiving document.
    pub fn block_instance_geometry(
        &self,
        reference: BlockReference,
    ) -> Result<Geometry, DocumentError> {
        Ok(Geometry::BlockInstance(in_catalog(
            &self.block_definitions,
            self.tolerance,
            reference,
        )?))
    }

    pub fn add_block_instance(
        &mut self,
        reference: BlockReference,
    ) -> Result<ObjectId, DocumentError> {
        self.add_block_instance_with_attributes(
            reference,
            ObjectAttributes::on_layer(self.current_layer),
        )
    }

    pub fn add_block_instance_with_attributes(
        &mut self,
        reference: BlockReference,
        attributes: ObjectAttributes,
    ) -> Result<ObjectId, DocumentError> {
        self.add_geometry_with_attributes(self.block_instance_geometry(reference)?, attributes)
    }

    pub(super) fn refresh_instance_geometry(
        &self,
        geometry: Geometry,
    ) -> Result<Geometry, DocumentError> {
        if let Geometry::BlockInstance(instance) = geometry {
            let current = self.block_instance_geometry(instance.reference)?;
            if current == Geometry::BlockInstance(instance.clone()) {
                Ok(Geometry::BlockInstance(instance))
            } else {
                Ok(current)
            }
        } else {
            Ok(geometry)
        }
    }
}

pub(super) fn stage_catalog_instances(
    document: &Document,
    definitions: &[BlockDefinition],
) -> Result<Vec<(usize, GeometrySnapshot)>, DocumentError> {
    document
        .objects
        .iter()
        .enumerate()
        .filter_map(|(index, object)| {
            let Geometry::BlockInstance(instance) = &*object.geometry else {
                return None;
            };
            Some(
                in_catalog(definitions, document.tolerance, instance.reference)
                    .map(|updated| (index, updated)),
            )
        })
        .filter_map(|result| match result {
            Ok((index, updated))
                if *document.objects[index].geometry
                    == Geometry::BlockInstance(updated.clone()) =>
            {
                None
            }
            Ok((index, updated)) => Some(Ok((index, Geometry::BlockInstance(updated).into()))),
            Err(error) => Some(Err(error)),
        })
        .collect()
}

pub(super) fn rescaled_instance(
    definitions: &[BlockDefinition],
    tolerance: Tolerance,
    instance: &BlockInstance,
    scale: f64,
) -> Result<Geometry, DocumentError> {
    let old = instance.reference.transform();
    let tolerance = if instance
        .members()
        .iter()
        .any(|member| matches!(&*member.geometry, Geometry::Brep(_)))
    {
        Tolerance::try_new(
            tolerance.absolute() * scale,
            tolerance.relative(),
            tolerance.angular(),
        )?
    } else {
        tolerance
    };
    let placement = AffineTransform3::try_new(old.linear_rows(), old.translation().scaled(scale)?)?;
    Ok(Geometry::BlockInstance(in_catalog(
        definitions,
        tolerance,
        BlockReference::try_new(instance.reference.definition(), placement)?,
    )?))
}

pub(super) fn exchange_catalog(
    document: &mut Document,
    stored: &mut Vec<BlockDefinition>,
    instances: &mut [(ObjectId, GeometrySnapshot)],
) -> Result<(), DocumentError> {
    let indices = instances
        .iter()
        .map(|(id, _)| {
            document
                .objects
                .iter()
                .position(|object| object.id == *id)
                .ok_or(DocumentError::HistoryInvariant(
                    "block catalog instance is missing",
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for ((_, geometry), index) in instances.iter_mut().zip(indices) {
        std::mem::swap(geometry, &mut document.objects[index].geometry);
    }
    std::mem::swap(stored, &mut document.block_definitions);
    Ok(())
}

#[cfg(test)]
mod tests;
