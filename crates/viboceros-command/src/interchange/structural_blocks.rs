//! Map file-local block indices onto independent document catalog IDs.
use super::*;
use viboceros_document::{
    BlockContent, BlockDefinition, BlockDefinitionId, BlockMember, BlockReference, GroupId, LayerId,
};
use viboceros_io::ThreeDmDefinition;

pub(super) fn import_definitions(
    document: &mut Document,
    definitions: Vec<ThreeDmDefinition>,
    layers: &[LayerId],
    groups: &[GroupId],
) -> Result<Vec<BlockDefinitionId>, CommandError> {
    let mut names = ImportNames::new(
        document
            .block_definitions()
            .map(|definition| definition.name()),
        true,
        "Imported Block",
    );
    let mut imported = definitions
        .iter()
        .map(|definition| BlockDefinition::new(names.allocate(&definition.name), Vec::new()))
        .collect::<Vec<_>>();
    let ids = imported
        .iter()
        .map(|definition| definition.id())
        .collect::<Vec<_>>();
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
                        ids[definition_index],
                        transform,
                    )?),
                    geometry => BlockContent::Geometry(document_geometry_from_3dm(geometry).into()),
                };
                let mut member = BlockMember::new(content, attributes).try_with_group_ids(
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
    Ok(ids)
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
    ids: &[BlockDefinitionId],
) -> Result<Geometry, CommandError> {
    match geometry {
        ThreeDmGeometry::InstanceReference {
            definition_index,
            transform,
        } => Ok(document
            .block_instance_geometry(BlockReference::try_new(ids[definition_index], transform)?)?),
        geometry => Ok(document_geometry_from_3dm(geometry)),
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
                        group_indices: member.group_ids().iter().map(|id| groups[id]).collect(),
                    })
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            Ok(ThreeDmDefinition {
                name: definition.name().into(),
                members,
            })
        })
        .collect()
}
