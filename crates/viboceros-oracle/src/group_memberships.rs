//! Ordered group attributes observed through real object commands.
use super::*;
use crate::object_source::ObjectSource;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GroupMembershipFixture {
    pub sources: Vec<ObjectSource>,
    pub groups: Vec<Vec<usize>>,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    Set { object: usize, groups: Vec<usize> },
    Add { group: usize, objects: Vec<usize> },
    DeleteGroup { group: usize },
    Select { objects: Vec<usize> },
    Command { name: ObjectCommand },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum ObjectCommand {
    Copy,
    Array,
    ArrayLinear,
    ArrayPolar,
    Ungroup,
    UngroupAll,
    Explode,
    ConvertToBeziers,
    Distribute,
}

pub(super) fn run(
    f: &GroupMembershipFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    validate(f)?;
    let mut document = Document::new(tolerance);
    let ids = f
        .sources
        .iter()
        .enumerate()
        .map(|(i, source)| {
            let geometry = source.geometry(tolerance)?;
            Ok(document.add_geometry_with_attributes(
                geometry,
                ObjectAttributes::on_layer(document.current_layer_id()).with_name(i.to_string()),
            )?)
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    let object_ids = |indices: &[usize]| {
        indices
            .iter()
            .map(|i| {
                ids.get(*i)
                    .copied()
                    .ok_or(ProbeError::FixtureInvariant("invalid group source index"))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let mut groups = Vec::new();
    for (i, members) in f.groups.iter().enumerate() {
        let group = document.add_empty_group(Some(format!("Group-{i}")))?;
        document.add_group_members(group, object_ids(members)?)?;
        groups.push(group);
    }
    let group_ids = |indices: &[usize]| {
        indices
            .iter()
            .map(|i| {
                groups
                    .get(*i)
                    .copied()
                    .ok_or(ProbeError::FixtureInvariant("invalid group index"))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let registry = CommandRegistry::with_builtins();
    let mut sources = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect::<BTreeMap<_, _>>();
    let mut states = vec![record(&document, &ids, &sources)?];
    for step in &f.steps {
        match step {
            Step::Set { object, groups } => {
                document
                    .set_object_group_memberships(object_ids(&[*object])?[0], group_ids(groups)?)?;
            }
            Step::Add { group, objects } => {
                document.add_group_members(group_ids(&[*group])?[0], object_ids(objects)?)?;
            }
            Step::DeleteGroup { group } => {
                document.remove_group(group_ids(&[*group])?[0])?;
            }
            Step::Select { objects } => {
                document.clear_selection();
                for id in object_ids(objects)? {
                    document.select_objects_direct([id], SelectionMode::Add)?;
                }
            }
            Step::Command { name } => {
                let candidates = document
                    .selected_object_ids()
                    .map(|id| sources[&id])
                    .collect::<BTreeSet<_>>();
                let command = match name {
                    ObjectCommand::Copy => "Copy 0,0,0 10,0,0".into(),
                    ObjectCommand::Array => "Array 2 1 1 10 0 0".into(),
                    ObjectCommand::ArrayLinear => "ArrayLinear 2 0,0,0 10,0,0".into(),
                    ObjectCommand::ArrayPolar => {
                        "ArrayPolar 2 0,0,0 180 Rotate=Yes ZOffset=0".into()
                    }
                    ObjectCommand::Distribute => {
                        "Distribute XAxis Mode=Gap Spacing=Automatic".into()
                    }
                    ObjectCommand::ConvertToBeziers => "ConvertToBeziers DeleteInput=Yes".into(),
                    _ => format!("{name:?}"),
                };
                registry.execute(&mut document, &command)?;
                for object in document.objects() {
                    if sources.contains_key(&object.id()) {
                        continue;
                    }
                    let source = object
                        .attributes()
                        .name()
                        .and_then(|n| n.parse::<usize>().ok())
                        .filter(|i| *i < ids.len())
                        .or_else(|| (candidates.len() == 1).then(|| *candidates.first().unwrap()))
                        .ok_or(ProbeError::FixtureInvariant(
                            "ambiguous group output source",
                        ))?;
                    sources.insert(object.id(), source);
                }
            }
        }
        states.push(record(&document, &ids, &sources)?);
    }
    Ok((json!({"states":states}), 0))
}

fn validate(f: &GroupMembershipFixture) -> Result<(), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid group membership fixture");
    if f.sources.is_empty() || f.sources.len() > 32 || f.groups.len() > 16 || f.steps.len() > 64 {
        return Err(invalid());
    }
    let indices = |values: &[usize], count, unique| {
        if values.iter().any(|i| *i >= count)
            || (unique && values.iter().collect::<BTreeSet<_>>().len() != values.len())
        {
            Err(invalid())
        } else {
            Ok(())
        }
    };
    for group in &f.groups {
        indices(group, f.sources.len(), true)?;
    }
    let mut live = (0..f.groups.len()).collect::<BTreeSet<_>>();
    for step in &f.steps {
        match step {
            Step::Set { object, groups } => {
                indices(&[*object], f.sources.len(), true)?;
                indices(groups, f.groups.len(), true)?;
                if groups.iter().any(|g| !live.contains(g)) {
                    return Err(invalid());
                }
            }
            Step::Add { group, objects } => {
                if !live.contains(group) {
                    return Err(invalid());
                }
                indices(objects, f.sources.len(), false)?;
            }
            Step::DeleteGroup { group } => {
                if !live.remove(group) {
                    return Err(invalid());
                }
            }
            Step::Select { objects } => indices(objects, f.sources.len(), true)?,
            Step::Command { .. } => {}
        }
    }
    Ok(())
}

fn record(
    document: &Document,
    original_ids: &[ObjectId],
    sources: &BTreeMap<ObjectId, usize>,
) -> Result<Value, ProbeError> {
    // Rhino's reused document reserves automatic names even after cleanup.
    // Compare their format and relative creation order, not that external counter.
    let mut automatic = document
        .groups()
        .filter_map(|group| {
            let suffix = group.name()?.strip_prefix("Group")?;
            (!suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()))
                .then(|| {
                    suffix
                        .parse::<u64>()
                        .ok()
                        .map(|number| (number, group.id()))
                })
                .flatten()
        })
        .collect::<Vec<_>>();
    automatic.sort_by_key(|(number, _)| *number);
    let automatic = automatic
        .into_iter()
        .enumerate()
        .map(|(i, (_, id))| (id, format!("CopyGroup-{i}")))
        .collect::<BTreeMap<_, _>>();
    let group_name = |id| {
        automatic
            .get(&id)
            .cloned()
            .unwrap_or_else(|| document.group(id).unwrap().name().unwrap().to_owned())
    };
    let mut objects = Vec::new();
    for object in document.objects() {
        let source = sources[&object.id()];
        let groups = object
            .group_ids()
            .iter()
            .map(|id| group_name(*id))
            .collect::<Vec<_>>();
        let (domain, points) = crate::distribute::sample(object.geometry())?;
        let retained = original_ids[source] == object.id();
        let selected = document.is_selected(object.id());
        let key = (source, points[0], groups.clone(), selected, retained);
        objects.push((key,json!({"source":source,"name":object.attributes().name(),"groups":groups,"domain":domain,"points":points,"selected":selected,"retained":retained})));
    }
    objects.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut groups = document
        .groups()
        .map(|group| {
            let mut members = group.members().map(|id| sources[&id]).collect::<Vec<_>>();
            members.sort_unstable();
            (group_name(group.id()), members)
        })
        .collect::<Vec<_>>();
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(
        json!({"objects":objects.into_iter().map(|(_,value)|value).collect::<Vec<_>>(),"groups":groups.into_iter().map(|(name,members)|json!({"name":name,"members":members})).collect::<Vec<_>>()}),
    )
}
