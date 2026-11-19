//! Retain archive definition IDs and remap references when importing another copy.
use super::*;
use viboceros_document::{
    BlockContent, BlockDefinition, BlockDefinitionId, BlockMember, BlockReference, GroupId, LayerId,
};
use viboceros_io::ThreeDmDefinition;

pub(super) struct ImportedDefinitions {
    ids: Vec<BlockDefinitionId>,
    remapped: BTreeMap<BlockDefinitionId, BlockDefinitionId>,
}

impl ImportedDefinitions {
    fn remap_geometry(&self, geometry: ThreeDmGeometry) -> Result<Geometry, CommandError> {
        if let ThreeDmGeometry::Leader(leader) = &geometry
            && let Some(id) = leader.style().leader_appearance().arrow_block_id()
            && let Some(mapped) = self.remapped.get(&BlockDefinitionId::from_bytes(id))
        {
            let style = leader
                .style()
                .with_leader_arrow_block(*mapped.uuid().as_bytes())
                .ok_or(viboceros_geometry::GeometryError::InvalidLeader)?;
            return Ok(Geometry::Leader(viboceros_geometry::Leader::try_styled(
                leader.plane(),
                leader.path().vertices().to_vec(),
                leader.text(),
                leader.rich_text(),
                style,
            )?));
        }
        Ok(document_geometry_from_3dm(geometry))
    }
}

pub(super) fn import_definitions(
    document: &mut Document,
    definitions: Vec<ThreeDmDefinition>,
    layers: &[LayerId],
    groups: &[GroupId],
) -> Result<ImportedDefinitions, CommandError> {
    let mut names = ImportNames::new(
        document
            .block_definitions()
            .map(|definition| definition.name()),
        true,
        "Imported Block",
    );
    let mut used = document
        .block_definitions()
        .map(BlockDefinition::id)
        .collect::<BTreeSet<_>>();
    let mut remapped = BTreeMap::new();
    let mut imported = definitions
        .iter()
        .map(|definition| {
            let original = BlockDefinitionId::from_uuid(definition.id);
            let name = names.allocate(&definition.name);
            let mut block = BlockDefinition::new(name, Vec::new());
            if used.insert(original) {
                return block.with_id(original);
            }
            while !used.insert(block.id()) {
                block = BlockDefinition::new(block.name(), Vec::new());
            }
            remapped.insert(original, block.id());
            block
        })
        .collect::<Vec<_>>();
    let ids = imported
        .iter()
        .map(|definition| definition.id())
        .collect::<Vec<_>>();
    let imports = ImportedDefinitions { ids, remapped };
    for (index, definition) in definitions.into_iter().enumerate() {
        let members = definition
            .members
            .into_iter()
            .map(|object| {
                let attributes = attributes(&object, layers[object.layer_index])?;
                let content = match object.geometry {
                    ThreeDmGeometry::InstanceReference {
                        definition_index,
                        transform,
                    } => BlockContent::Reference(BlockReference::try_new(
                        imports.ids[definition_index],
                        transform,
                    )?),
                    geometry => BlockContent::Geometry(imports.remap_geometry(geometry)?.into()),
                };
                let mut member = BlockMember::new(content, attributes)
                    .with_id(viboceros_document::ObjectId::from_uuid(object.id))
                    .try_with_group_ids(
                        object
                            .group_indices
                            .iter()
                            .map(|index| groups[*index])
                            .collect(),
                    )?;
                for (key, value) in object.geometry_user_text {
                    member = member.try_with_geometry_user_text(key, value)?;
                }
                Ok(member)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        imported[index] = imported[index].clone().with_members(members);
    }
    let catalog = document
        .block_definitions()
        .cloned()
        .chain(imported)
        .collect();
    document.set_block_definitions(catalog)?;
    Ok(imports)
}
fn attributes(object: &ThreeDmObject, layer: LayerId) -> Result<ObjectAttributes, CommandError> {
    let mut attributes = ObjectAttributes::on_layer(layer)
        .with_object_color(ColorRgb::new(
            object.object_color[0],
            object.object_color[1],
            object.object_color[2],
        ))
        .with_color_source(document_color_source_from_3dm(object.color_source))
        .with_file_state(object.visible, object.locked)
        .try_with_opacity(object.opacity)?
        .try_with_wire_density(object.wire_density)?;
    if let Some(name) = &object.name {
        attributes = attributes.with_name(name);
    }
    for (key, value) in &object.user_text {
        attributes = attributes.try_with_user_text(key, value)?;
    }
    Ok(attributes)
}
pub(super) fn import_geometry(
    document: &Document,
    geometry: ThreeDmGeometry,
    imports: &ImportedDefinitions,
) -> Result<Geometry, CommandError> {
    match geometry {
        ThreeDmGeometry::InstanceReference {
            definition_index,
            transform,
        } => Ok(document.block_instance_geometry(BlockReference::try_new(
            imports.ids[definition_index],
            transform,
        )?)?),
        geometry => imports.remap_geometry(geometry),
    }
}
pub(super) fn export_geometry(
    geometry: &Geometry,
    definitions: &BTreeMap<BlockDefinitionId, usize>,
) -> Result<ThreeDmGeometry, CommandError> {
    match geometry {
        Geometry::BlockInstance(instance) => {
            let reference = instance.reference();
            Ok(ThreeDmGeometry::InstanceReference {
                definition_index: definitions[&reference.definition()],
                transform: reference.transform(),
            })
        }
        geometry => geometry_to_3dm(geometry),
    }
}
pub(super) fn export_definitions(
    document: &Document,
    definitions: &BTreeMap<BlockDefinitionId, usize>,
    layers: &BTreeMap<LayerId, usize>,
    groups: &BTreeMap<GroupId, usize>,
) -> Result<Vec<ThreeDmDefinition>, CommandError> {
    document
        .block_definitions()
        .map(|definition| {
            let members = definition
                .members()
                .iter()
                .map(|member| {
                    let geometry = match member.content() {
                        BlockContent::Reference(reference) => ThreeDmGeometry::InstanceReference {
                            definition_index: definitions[&reference.definition()],
                            transform: reference.transform(),
                        },
                        BlockContent::Geometry(geometry) => geometry_to_3dm(geometry)?,
                    };
                    let attributes = member.attributes();
                    let color = attributes.object_color();
                    Ok(ThreeDmObject {
                        id: member.id().uuid(),
                        geometry,
                        layer_index: layers[&attributes.layer_id()],
                        name: attributes.name().map(str::to_owned),
                        user_text: attributes.user_text().clone(),
                        geometry_user_text: member.geometry_user_text().clone(),
                        visible: attributes.is_visible(),
                        locked: attributes.is_locked(),
                        object_color: [color.red, color.green, color.blue],
                        color_source: three_dm_color_source_from_document(
                            attributes.color_source(),
                        ),
                        wire_density: attributes.wire_density(),
                        opacity: attributes.opacity(),
                        group_indices: member.group_ids().iter().map(|id| groups[id]).collect(),
                    })
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            Ok(ThreeDmDefinition {
                id: definition.id().uuid(),
                name: definition.name().into(),
                members,
            })
        })
        .collect()
}
