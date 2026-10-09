//! UUID-independent block graph, placement and expansion records.
use super::*;
use crate::object_source::ObjectSource;
use viboceros_document::{BlockContent, BlockReference, GroupId};

const MAX_HANDLES: usize = 4096;
const MAX_RECORDS: usize = 65_536;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BlockWorkflowFixture {
    pub sources: Vec<ObjectSource>,
    #[serde(default)]
    pub attributes: Vec<BlockAttributes>,
    #[serde(default)]
    pub record_groups: bool,
    #[serde(default)]
    pub record_management: bool,
    #[serde(default)]
    pub record_states: bool,
    pub steps: Vec<BlockStep>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BlockColorSource {
    #[default]
    Layer,
    Object,
    Parent,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BlockAttributes {
    pub name: Option<String>,
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub color_source: BlockColorSource,
    #[serde(default)]
    pub user_text: BTreeMap<String, String>,
    #[serde(default)]
    pub geometry_user_text: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum BlockStep {
    Create {
        name: String,
        base: [f64; 3],
        sources: Vec<usize>,
        #[serde(default = "sdk_creation_api")]
        api: BlockExplosionApi,
    },
    Insert {
        name: String,
        transform: [[f64; 4]; 4],
        #[serde(default)]
        attributes: BlockAttributes,
    },
    Explode {
        object: usize,
        #[serde(default)]
        recursive: bool,
        #[serde(default)]
        api: BlockExplosionApi,
        #[serde(default)]
        group_output: bool,
    },
    Group {
        objects: Vec<usize>,
    },
    ExplodeBatch {
        objects: Vec<usize>,
        #[serde(default)]
        group_output: bool,
    },
    RenameDefinition {
        name: String,
        new_name: String,
    },
    MakeUnique {
        objects: Vec<usize>,
        name: String,
    },
    DuplicateDefinition {
        name: String,
        new_name: String,
    },
    ReplaceBlock {
        objects: Vec<usize>,
        name: String,
        #[serde(default)]
        all_instances: bool,
    },
    ObjectState {
        objects: Vec<usize>,
        mode: String,
    },
    DeleteDefinition {
        name: String,
        #[serde(default)]
        expect_failure: bool,
    },
}

fn sdk_creation_api() -> BlockExplosionApi {
    BlockExplosionApi::Sdk
}

/// Selects the native reference entrypoint. Local records use the document kernel.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BlockExplosionApi {
    Sdk,
    #[default]
    Command,
}

pub(super) fn run(
    f: &BlockWorkflowFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid block workflow");
    if !(1..=32).contains(&f.sources.len())
        || !(1..=64).contains(&f.steps.len())
        || (!f.attributes.is_empty() && f.attributes.len() != f.sources.len())
    {
        return Err(invalid());
    }
    let mut document = Document::new(tolerance);
    let source_layer = document.current_layer_id();
    document.rename_layer(source_layer, "Source")?;
    let current_layer = document.add_layer("Current", ColorRgb::BLACK)?;
    document.set_current_layer(current_layer)?;
    let mut handles = Vec::new();
    for (i, source) in f.sources.iter().enumerate() {
        let default = BlockAttributes::default();
        let spec = f.attributes.get(i).unwrap_or(&default);
        let attributes = attributes(spec, source_layer)?
            .with_name(spec.name.as_deref().unwrap_or(&format!("source-{i}")));
        let id = document.add_geometry_with_attributes(source.geometry(tolerance)?, attributes)?;
        attach_text(&mut document, id, spec)?;
        handles.push(id);
    }
    let mut records_left = MAX_RECORDS;
    let mut snapshots = vec![snapshot(
        &document,
        &handles,
        &mut records_left,
        f.record_groups,
        f.record_management,
        f.record_states,
    )?];
    for step in &f.steps {
        let mut succeeded = None;
        let outputs = match step {
            BlockStep::Create {
                name,
                base,
                sources,
                api,
            } => {
                valid_name(name)?;
                if *api == BlockExplosionApi::Command
                    && (document.block_definition_by_name(name).is_some()
                        || !name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')))
                {
                    return Err(invalid());
                }
                if sources.is_empty()
                    || sources.iter().collect::<BTreeSet<_>>().len() != sources.len()
                {
                    return Err(invalid());
                }
                let ids = sources
                    .iter()
                    .map(|i| live(&document, &handles, *i))
                    .collect::<Result<Vec<_>, _>>()?;
                let (_, id) =
                    document.create_block_from_objects(name, Point3::try_from(*base)?, ids)?;
                vec![id]
            }
            BlockStep::Insert {
                name,
                transform,
                attributes: spec,
            } => {
                valid_name(name)?;
                let definition = document
                    .block_definition_by_name(name)
                    .ok_or_else(invalid)?;
                let reference = BlockReference::try_new(definition.id(), affine(*transform)?)?;
                let id = document.add_block_instance_with_attributes(
                    reference,
                    attributes(spec, current_layer)?,
                )?;
                attach_text(&mut document, id, spec)?;
                vec![id]
            }
            BlockStep::Explode {
                object,
                recursive,
                api,
                group_output,
            } => {
                if *group_output && (!recursive || *api == BlockExplosionApi::Sdk) {
                    return Err(invalid());
                }
                let id = live(&document, &handles, *object)?;
                let plan = document.prepare_block_explosion(
                    id,
                    *recursive,
                    MAX_HANDLES - handles.len(),
                )?;
                document.commit_block_explosions(vec![plan], *group_output)?
            }
            BlockStep::Group { objects } => {
                if objects.is_empty()
                    || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
                {
                    return Err(invalid());
                }
                let ids = objects
                    .iter()
                    .map(|i| live(&document, &handles, *i))
                    .collect::<Result<Vec<_>, _>>()?;
                document.add_group(None, ids)?;
                Vec::new()
            }
            BlockStep::ExplodeBatch {
                objects,
                group_output,
            } => {
                if objects.is_empty()
                    || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
                {
                    return Err(invalid());
                }
                let mut indices = objects.clone();
                indices.sort_unstable();
                let mut budget = MAX_HANDLES - handles.len();
                let mut plans = Vec::new();
                for index in indices {
                    let plan = document.prepare_block_explosion(
                        live(&document, &handles, index)?,
                        true,
                        budget,
                    )?;
                    budget -= plan.output_count();
                    plans.push(plan);
                }
                document.commit_block_explosions(plans, *group_output)?
            }
            BlockStep::RenameDefinition { name, new_name } => {
                valid_name(new_name)?;
                let id = document
                    .block_definition_by_name(name)
                    .ok_or_else(invalid)?
                    .id();
                document.rename_block_definition(id, new_name)?;
                Vec::new()
            }
            BlockStep::MakeUnique { objects, name } => {
                valid_name(name)?;
                if objects.is_empty()
                    || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
                {
                    return Err(invalid());
                }
                let ids = objects
                    .iter()
                    .map(|index| live(&document, &handles, *index))
                    .collect::<Result<Vec<_>, _>>()?;
                document.make_block_instances_unique(name, ids)?;
                Vec::new()
            }
            BlockStep::DuplicateDefinition { name, new_name } => {
                valid_name(new_name)?;
                let id = document
                    .block_definition_by_name(name)
                    .ok_or_else(invalid)?
                    .id();
                document.duplicate_block_definition(id, new_name)?;
                Vec::new()
            }
            BlockStep::ReplaceBlock {
                objects,
                name,
                all_instances,
            } => {
                if objects.is_empty()
                    || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
                {
                    return Err(invalid());
                }
                let target = document
                    .block_definition_by_name(name)
                    .ok_or_else(invalid)?
                    .id();
                let ids = objects
                    .iter()
                    .map(|i| live(&document, &handles, *i))
                    .collect::<Result<Vec<_>, _>>()?;
                document.replace_block_instances(target, ids, *all_instances)?;
                Vec::new()
            }
            BlockStep::ObjectState { objects, mode } => {
                if objects.is_empty()
                    || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
                {
                    return Err(invalid());
                }
                let ids = objects
                    .iter()
                    .map(|i| live(&document, &handles, *i))
                    .collect::<Result<Vec<_>, _>>()?;
                match mode.as_str() {
                    "normal" => {
                        document.set_objects_visibility(ids.clone(), true)?;
                        document.set_objects_locked(ids, false)?;
                    }
                    "hidden" => {
                        document.set_objects_visibility(ids, false)?;
                    }
                    "locked" => {
                        document.set_objects_locked(ids, true)?;
                    }
                    _ => return Err(invalid()),
                }
                Vec::new()
            }
            BlockStep::DeleteDefinition {
                name,
                expect_failure,
            } => {
                let id = document
                    .block_definition_by_name(name)
                    .ok_or_else(invalid)?
                    .id();
                let result = document.delete_block_definition_and_instances(id);
                succeeded = Some(result.is_ok());
                if *expect_failure {
                    if result.is_ok() {
                        return Err(invalid());
                    }
                } else {
                    result?;
                }
                Vec::new()
            }
        };
        if handles.len() + outputs.len() > MAX_HANDLES {
            return Err(invalid());
        }
        let output_handles = (handles.len()..handles.len() + outputs.len()).collect::<Vec<_>>();
        handles.extend(outputs);
        let mut value = snapshot(
            &document,
            &handles,
            &mut records_left,
            f.record_groups,
            f.record_management,
            f.record_states,
        )?;
        value["outputs"] = json!(output_handles);
        if let Some(succeeded) = succeeded {
            value["succeeded"] = json!(succeeded);
        }
        snapshots.push(value);
    }
    Ok((json!({"states": snapshots}), 0))
}

fn valid_name(name: &str) -> Result<(), ProbeError> {
    if name.trim().is_empty() || name.len() > 128 || name.trim() != name || name.contains('\0') {
        return Err(ProbeError::FixtureInvariant("invalid block workflow name"));
    }
    Ok(())
}

fn live(document: &Document, handles: &[ObjectId], index: usize) -> Result<ObjectId, ProbeError> {
    handles
        .get(index)
        .copied()
        .filter(|id| document.object(*id).is_some())
        .ok_or(ProbeError::FixtureInvariant(
            "block handle is absent or deleted",
        ))
}

fn affine(matrix: [[f64; 4]; 4]) -> Result<AffineTransform3, ProbeError> {
    if matrix[3] != [0., 0., 0., 1.] {
        return Err(ProbeError::FixtureInvariant("block matrix must be affine"));
    }
    Ok(AffineTransform3::try_new(
        std::array::from_fn(|r| std::array::from_fn(|c| matrix[r][c])),
        Vector3::try_new(matrix[0][3], matrix[1][3], matrix[2][3])?,
    )?)
}

fn attributes(spec: &BlockAttributes, layer: LayerId) -> Result<ObjectAttributes, ProbeError> {
    let [r, g, b] = spec.color.unwrap_or([0, 0, 0]);
    let mut result = ObjectAttributes::on_layer(layer)
        .with_object_color(ColorRgb::new(r, g, b))
        .with_color_source(match spec.color_source {
            BlockColorSource::Layer => ObjectColorSource::Layer,
            BlockColorSource::Object => ObjectColorSource::Object,
            BlockColorSource::Parent => ObjectColorSource::Parent,
        });
    if let Some(name) = &spec.name {
        result = result.with_name(name);
    }
    for (key, value) in &spec.user_text {
        result = result.try_with_user_text(key, value)?;
    }
    Ok(result)
}

fn attach_text(
    document: &mut Document,
    id: ObjectId,
    spec: &BlockAttributes,
) -> Result<(), ProbeError> {
    for (key, value) in &spec.geometry_user_text {
        document.set_object_geometry_user_text([id], key, Some(value))?;
    }
    Ok(())
}

fn attribute_record(
    document: &Document,
    a: &ObjectAttributes,
    text: &BTreeMap<String, String>,
) -> Value {
    let color = a.object_color();
    json!({"name": a.name(), "layer": document.layer(a.layer_id()).unwrap().name(),
        "color": [color.red, color.green, color.blue], "color_source": match a.color_source() {
            ObjectColorSource::Layer => "layer", ObjectColorSource::Object => "object",
            ObjectColorSource::Parent => "parent", ObjectColorSource::Material => "material",
        }, "user_text": a.user_text(), "geometry_user_text": text})
}

fn spend(left: &mut usize, count: usize) -> Result<(), ProbeError> {
    *left = left.checked_sub(count).ok_or(ProbeError::FixtureInvariant(
        "block recording budget exceeded",
    ))?;
    Ok(())
}

fn reference_record(document: &Document, reference: BlockReference) -> Value {
    let t = reference.transform();
    let linear = t.linear_rows();
    let translation = t.translation().to_array();
    let mut matrix = [[0.; 4]; 4];
    for r in 0..3 {
        matrix[r][..3].copy_from_slice(&linear[r]);
        matrix[r][3] = translation[r];
    }
    matrix[3][3] = 1.;
    json!({"kind": "block", "definition": document.block_definition(reference.definition()).unwrap().name(), "transform": matrix})
}

fn geometry_record(
    document: &Document,
    geometry: &Geometry,
    left: &mut usize,
    groups: Option<&BTreeMap<GroupId, usize>>,
) -> Result<Value, ProbeError> {
    spend(left, 1)?;
    if let Geometry::BlockInstance(instance) = geometry {
        let mut value = reference_record(document, instance.reference());
        let mut leaves = Vec::new();
        for member in instance.members() {
            let mut record = json!({"path": member.path.iter().map(|p| json!([
                document.block_definition(p.definition).unwrap().name(), p.member_index])).collect::<Vec<_>>(),
                "attributes": attribute_record(document, &member.attributes, &member.geometry_user_text),
                "geometry": geometry_record(document, &member.geometry, left, groups)?});
            if let Some(groups) = groups {
                record["groups"] = json!(
                    member
                        .group_ids
                        .iter()
                        .map(|id| groups[id])
                        .collect::<Vec<_>>()
                );
            }
            leaves.push(record);
        }
        value["leaves"] = json!(leaves);
        return Ok(value);
    }
    let (domain, points) = object_layout::sample(geometry)?;
    let kind = match geometry {
        Geometry::Point(_) => "point",
        Geometry::PointCloud(_) => "point_cloud",
        Geometry::Mesh(_) => "mesh",
        Geometry::NurbsSurface(_) => "surface",
        Geometry::Brep(_) => "brep",
        _ => "curve",
    };
    Ok(json!({"kind": kind, "domain": domain, "points": points}))
}

fn snapshot(
    document: &Document,
    handles: &[ObjectId],
    left: &mut usize,
    record_groups: bool,
    record_management: bool,
    record_states: bool,
) -> Result<Value, ProbeError> {
    let group_index = document
        .groups()
        .enumerate()
        .map(|(i, g)| (g.id(), i))
        .collect::<BTreeMap<_, _>>();
    let groups = record_groups.then_some(&group_index);
    let objects = handles
        .iter()
        .enumerate()
        .filter_map(|(index, id)| document.object(*id).map(|o| (index, o)))
        .map(|(index, o)| {
            let mut value = json!({"handle": index,
            "attributes": attribute_record(document, o.attributes(), o.geometry_user_text()),
            "geometry": geometry_record(document, o.geometry(), left, groups)?});
            if record_groups {
                value["groups"] = json!(
                    o.group_ids()
                        .iter()
                        .map(|id| group_index[id])
                        .collect::<Vec<_>>()
                );
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    let mut definitions = Vec::new();
    let management = if record_management {
        document
            .block_definition_info()?
            .into_iter()
            .map(|i| (i.id, i))
            .collect::<BTreeMap<_, _>>()
    } else {
        BTreeMap::new()
    };
    for definition in document.block_definitions() {
        let mut members = Vec::new();
        for member in definition.members() {
            let geometry = match member.content() {
                BlockContent::Geometry(g) => geometry_record(document, g, left, groups)?,
                BlockContent::Reference(r) => {
                    spend(left, 1)?;
                    reference_record(document, *r)
                }
            };
            let mut value = json!({"attributes": attribute_record(document, member.attributes(), member.geometry_user_text()), "geometry": geometry});
            if record_groups {
                value["groups"] = json!(
                    member
                        .group_ids()
                        .iter()
                        .map(|id| group_index[id])
                        .collect::<Vec<_>>()
                );
            }
            members.push(value);
        }
        let mut record = json!({"name": definition.name(), "members": members});
        if record_management {
            let i = &management[&definition.id()];
            record["usage"] = json!({"top_level":i.top_level_instances,"nested":i.nested_instances,"total":i.total_instances(),"definition_references":i.definition_references});
        }
        definitions.push(record);
    }
    definitions.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let mut value = json!({"objects": objects, "definitions": definitions});
    if record_states {
        value["object_states"]=json!(handles.iter().enumerate().filter_map(|(i,id)|document.object(*id).map(|o|json!({"handle":i,"visible":o.attributes().is_visible(),"locked":o.attributes().is_locked()}))).collect::<Vec<_>>());
    }
    if record_groups {
        value["groups"] = json!(document.groups().enumerate().map(|(i,g)| json!({"index":i,"objects": handles.iter().enumerate().filter_map(|(j,id)| g.members().any(|member| member==*id).then_some(j)).collect::<Vec<_>>()})).collect::<Vec<_>>());
    }
    Ok(value)
}

#[cfg(test)]
mod tests;
