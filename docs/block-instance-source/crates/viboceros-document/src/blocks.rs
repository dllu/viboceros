//! Shared, immutable block definitions in document-unit coordinates.
use std::{collections::BTreeMap, sync::Arc};

use super::{
    BlockDefinitionId, Document, DocumentError, Edit, Geometry, GeometrySnapshot, ObjectAttributes,
    validate_user_text,
};
use viboceros_geometry::{AffineTransform3, GeometryError, Point3, Tolerance};

const MAX_DEFINITIONS: usize = 10_000;
const MAX_MEMBERS: usize = 100_000;
const MAX_DEPTH: usize = 64;
const MAX_RESOLUTION_VISITS: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockReference {
    definition: BlockDefinitionId,
    transform: AffineTransform3,
}

impl BlockReference {
    /// Maps definition-local coordinates into its parent's coordinates.
    /// Reflections are allowed; singular and nonfinite placements are rejected.
    pub fn try_new(
        definition: BlockDefinitionId,
        transform: AffineTransform3,
    ) -> Result<Self, DocumentError> {
        transform.orientation_reversing()?;
        Ok(Self {
            definition,
            transform,
        })
    }

    pub const fn definition(self) -> BlockDefinitionId {
        self.definition
    }
    pub const fn transform(self) -> AffineTransform3 {
        self.transform
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockContent {
    Geometry(GeometrySnapshot),
    Reference(BlockReference),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockMember {
    content: BlockContent,
    attributes: ObjectAttributes,
    geometry_user_text: BTreeMap<String, String>,
}

impl BlockMember {
    pub fn new(content: BlockContent, attributes: ObjectAttributes) -> Self {
        Self {
            content,
            attributes,
            geometry_user_text: BTreeMap::new(),
        }
    }
    pub fn content(&self) -> &BlockContent {
        &self.content
    }
    pub fn attributes(&self) -> &ObjectAttributes {
        &self.attributes
    }
    pub fn geometry_user_text(&self) -> &BTreeMap<String, String> {
        &self.geometry_user_text
    }

    pub fn try_with_geometry_user_text(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, DocumentError> {
        let key = key.into();
        let value = value.into();
        validate_user_text(&key, Some(&value))?;
        super::set_user_text_pair(&mut self.geometry_user_text, &key, Some(&value));
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockDefinition {
    id: BlockDefinitionId,
    name: String,
    members: Arc<Vec<BlockMember>>,
}

impl BlockDefinition {
    /// Catalog admission validates names, references, layers and resource limits.
    /// Members are expressed in definition-local document units.
    pub fn new(name: impl Into<String>, members: Vec<BlockMember>) -> Self {
        Self {
            id: BlockDefinitionId::new(),
            name: name.into().trim().to_owned(),
            members: Arc::new(members),
        }
    }
    pub const fn id(&self) -> BlockDefinitionId {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn members(&self) -> &[BlockMember] {
        &self.members
    }
    pub fn with_members(mut self, members: Vec<BlockMember>) -> Self {
        self.members = Arc::new(members);
        self
    }
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into().trim().to_owned();
        self
    }
}

/// An ordered root-to-leaf path. Repeated uses of one definition remain distinct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockMemberLocation {
    pub definition: BlockDefinitionId,
    pub member_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedBlockMember {
    pub geometry: GeometrySnapshot,
    /// Raw leaf attributes. Parent color/visibility inheritance is not applied.
    pub attributes: ObjectAttributes,
    pub geometry_user_text: BTreeMap<String, String>,
    pub path: Vec<BlockMemberLocation>,
}

impl Document {
    pub fn block_definitions(&self) -> impl ExactSizeIterator<Item = &BlockDefinition> {
        self.block_definitions.iter()
    }

    pub fn block_definition(&self, id: BlockDefinitionId) -> Option<&BlockDefinition> {
        self.block_definitions
            .iter()
            .find(|definition| definition.id == id)
    }

    /// Atomically replaces the catalog, including batches with forward references.
    /// Names are unique under ASCII case folding, following the layer API policy.
    /// This is a document API, not a Rhino command emulation.
    pub fn set_block_definitions(
        &mut self,
        definitions: Vec<BlockDefinition>,
    ) -> Result<bool, DocumentError> {
        validate_catalog(self, &definitions)?;
        if self.block_definitions == definitions {
            return Ok(false);
        }
        let staged = super::block_instances::stage_catalog_instances(self, &definitions)?;
        let instances = staged
            .into_iter()
            .map(|(index, geometry)| {
                let object = &mut self.objects[index];
                (object.id, std::mem::replace(&mut object.geometry, geometry))
            })
            .collect();
        let stored = std::mem::replace(&mut self.block_definitions, definitions);
        self.record_edit(
            "Block definitions",
            Edit::BlockDefinitionsChanged { stored, instances },
        );
        Ok(true)
    }

    pub fn add_block_definition(
        &mut self,
        name: impl Into<String>,
        members: Vec<BlockMember>,
    ) -> Result<BlockDefinitionId, DocumentError> {
        let definition = BlockDefinition::new(name, members);
        let id = definition.id;
        let mut definitions = self.block_definitions.clone();
        definitions.push(definition);
        self.set_block_definitions(definitions)?;
        Ok(id)
    }

    pub fn replace_block_definition_members(
        &mut self,
        id: BlockDefinitionId,
        members: Vec<BlockMember>,
    ) -> Result<bool, DocumentError> {
        let mut definitions = self.block_definitions.clone();
        let definition = definitions
            .iter_mut()
            .find(|definition| definition.id == id)
            .ok_or(DocumentError::BlockDefinitionNotFound(id))?;
        definition.members = Arc::new(members);
        self.set_block_definitions(definitions)
    }

    /// Rejects deletion while another definition references this definition.
    pub fn remove_block_definition(&mut self, id: BlockDefinitionId) -> Result<(), DocumentError> {
        let mut definitions = self.block_definitions.clone();
        let index = definitions
            .iter()
            .position(|definition| definition.id == id)
            .ok_or(DocumentError::BlockDefinitionNotFound(id))?;
        definitions.remove(index);
        self.set_block_definitions(definitions)?;
        Ok(())
    }

    /// Resolves a placement without modifying stored definitions or history.
    /// Any failed leaf transformation rejects the complete result. Hidden/locked
    /// leaves are retained, with raw metadata and paths for later consumers.
    pub fn resolve_block(
        &self,
        reference: BlockReference,
    ) -> Result<Vec<ResolvedBlockMember>, DocumentError> {
        resolve_in_catalog(&self.block_definitions, self.tolerance, reference)
    }
}

pub(super) fn resolve_in_catalog(
    definitions: &[BlockDefinition],
    tolerance: Tolerance,
    reference: BlockReference,
) -> Result<Vec<ResolvedBlockMember>, DocumentError> {
    let index = definition_index(definitions);
    let mut output = Vec::new();
    let mut path = Vec::new();
    let mut visits = 0;
    resolve(
        tolerance,
        &index,
        reference,
        &mut path,
        &mut visits,
        &mut output,
    )?;
    Ok(output)
}

fn definition_index(
    definitions: &[BlockDefinition],
) -> BTreeMap<BlockDefinitionId, &BlockDefinition> {
    definitions
        .iter()
        .map(|definition| (definition.id, definition))
        .collect()
}

fn validate_catalog(
    document: &Document,
    definitions: &[BlockDefinition],
) -> Result<(), DocumentError> {
    if definitions.len() > MAX_DEFINITIONS {
        return Err(invalid("definition limit exceeded"));
    }
    let index = definition_index(definitions);
    if index.len() != definitions.len() {
        return Err(invalid("duplicate definition ID"));
    }
    let mut names = std::collections::BTreeSet::new();
    let mut members = 0;
    for definition in definitions {
        if definition.name.is_empty() || definition.name.contains('\0') {
            return Err(invalid("definition name is empty or contains NUL"));
        }
        if !names.insert(definition.name.to_ascii_lowercase()) {
            return Err(invalid("duplicate definition name"));
        }
        if definition.members.len() > MAX_MEMBERS - members {
            return Err(invalid("member limit exceeded"));
        }
        members += definition.members.len();
        for member in definition.members() {
            if matches!(&member.content, BlockContent::Geometry(geometry) if matches!(&**geometry, Geometry::BlockInstance(_)))
            {
                return Err(invalid(
                    "instance geometry must be stored as a definition reference",
                ));
            }
            document
                .layer(member.attributes.layer_id())
                .ok_or(DocumentError::LayerNotFound(member.attributes.layer_id()))?;
            if let BlockContent::Reference(reference) = member.content {
                reference.transform.orientation_reversing()?;
                if !index.contains_key(&reference.definition) {
                    return Err(DocumentError::BlockDefinitionNotFound(reference.definition));
                }
            }
        }
    }
    let mut heights = BTreeMap::new();
    let mut active = std::collections::BTreeSet::new();
    for definition in definitions {
        height(definition.id, &index, &mut heights, &mut active)?;
    }
    Ok(())
}

fn height(
    id: BlockDefinitionId,
    index: &BTreeMap<BlockDefinitionId, &BlockDefinition>,
    heights: &mut BTreeMap<BlockDefinitionId, usize>,
    active: &mut std::collections::BTreeSet<BlockDefinitionId>,
) -> Result<usize, DocumentError> {
    if let Some(height) = heights.get(&id) {
        return Ok(*height);
    }
    if !active.insert(id) {
        return Err(invalid("cyclic definition graph"));
    }
    if active.len() > MAX_DEPTH {
        return Err(invalid("definition nesting limit exceeded"));
    }
    let mut value = 1;
    for member in index[&id].members() {
        if let BlockContent::Reference(reference) = member.content {
            value = value.max(1 + height(reference.definition, index, heights, active)?);
        }
    }
    if value > MAX_DEPTH {
        return Err(invalid("definition nesting limit exceeded"));
    }
    active.remove(&id);
    heights.insert(id, value);
    Ok(value)
}

fn resolve(
    tolerance: Tolerance,
    index: &BTreeMap<BlockDefinitionId, &BlockDefinition>,
    reference: BlockReference,
    path: &mut Vec<BlockMemberLocation>,
    visits: &mut usize,
    output: &mut Vec<ResolvedBlockMember>,
) -> Result<(), DocumentError> {
    *visits += 1;
    if *visits > MAX_RESOLUTION_VISITS {
        return Err(invalid("placement visit limit exceeded"));
    }
    if path.len() >= MAX_DEPTH {
        return Err(invalid("placement nesting limit exceeded"));
    }
    let definition = index
        .get(&reference.definition)
        .ok_or(DocumentError::BlockDefinitionNotFound(reference.definition))?;
    for (member_index, member) in definition.members().iter().enumerate() {
        *visits += 1;
        if *visits > MAX_RESOLUTION_VISITS {
            return Err(invalid("placement visit limit exceeded"));
        }
        path.push(BlockMemberLocation {
            definition: definition.id,
            member_index,
        });
        match &member.content {
            BlockContent::Geometry(geometry) => {
                if output.len() >= MAX_MEMBERS {
                    return Err(invalid("placed geometry limit exceeded"));
                }
                let geometry = if reference.transform == AffineTransform3::identity() {
                    geometry.clone()
                } else {
                    let tolerance = transformation_tolerance(geometry, tolerance, 1.0)?;
                    geometry.transformed(reference.transform, tolerance)?.into()
                };
                output.push(ResolvedBlockMember {
                    geometry,
                    attributes: member.attributes.clone(),
                    geometry_user_text: member.geometry_user_text.clone(),
                    path: path.clone(),
                });
            }
            BlockContent::Reference(child) => {
                let placed = BlockReference::try_new(
                    child.definition,
                    child.transform.then(reference.transform)?,
                )?;
                resolve(tolerance, index, placed, path, visits, output)?;
            }
        }
        path.pop();
    }
    Ok(())
}

pub(super) fn transformation_tolerance(
    geometry: &Geometry,
    tolerance: Tolerance,
    scale: f64,
) -> Result<Tolerance, GeometryError> {
    Ok(match geometry {
        Geometry::Brep(_) => Tolerance::try_new(
            tolerance.absolute() * scale,
            tolerance.relative(),
            tolerance.angular(),
        )?,
        Geometry::Mesh(_) => Tolerance::MESH_VALIDATION,
        _ => Tolerance::NUMERICAL_VALIDATION,
    })
}

pub(super) fn rescaled_definitions(
    document: &Document,
    scale: f64,
) -> Result<Vec<BlockDefinition>, DocumentError> {
    let transform = AffineTransform3::try_uniform_scale(Point3::try_new(0., 0., 0.)?, scale)?;
    document
        .block_definitions
        .iter()
        .map(|definition| {
            let members = definition
                .members()
                .iter()
                .map(|member| {
                    let content = match &member.content {
                        BlockContent::Geometry(geometry) => BlockContent::Geometry(
                            geometry
                                .transformed(
                                    transform,
                                    transformation_tolerance(geometry, document.tolerance, scale)?,
                                )?
                                .into(),
                        ),
                        BlockContent::Reference(reference) => {
                            BlockContent::Reference(BlockReference::try_new(
                                reference.definition,
                                AffineTransform3::try_new(
                                    reference.transform.linear_rows(),
                                    reference.transform.translation().scaled(scale)?,
                                )?,
                            )?)
                        }
                    };
                    Ok(BlockMember {
                        content,
                        ..member.clone()
                    })
                })
                .collect::<Result<Vec<_>, DocumentError>>()?;
            Ok(definition.clone().with_members(members))
        })
        .collect()
}

pub(super) fn invalid(reason: &'static str) -> DocumentError {
    DocumentError::InvalidBlockCatalog(reason)
}

#[cfg(test)]
mod tests;
