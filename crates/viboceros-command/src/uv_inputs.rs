//! Ephemeral curve ranges retain source identity without editing the document.
use super::*;
use std::borrow::Cow;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(super) struct SubcurveInput {
    pub(super) object: ObjectId,
    pub(super) parameters: [Real; 2],
    arc_length: bool,
}

pub(super) struct Input<'a> {
    pub(super) object: ObjectId,
    pub(super) geometry: Cow<'a, Geometry>,
    pub(super) temporary: bool,
}

impl SubcurveInput {
    pub(super) fn parse(value: &str, usage: &'static str) -> Result<Self, CommandError> {
        let values = value.split(',').collect::<Vec<_>>();
        let [id, start, end] = values.as_slice() else {
            return Err(CommandError::Usage(usage));
        };
        Ok(Self {
            object: id.parse().map_err(|_| CommandError::Usage(usage))?,
            parameters: [parse_finite_real(start)?, parse_finite_real(end)?],
            arc_length: false,
        })
    }
    pub(super) fn parse_length(value: &str, usage: &'static str) -> Result<Self, CommandError> {
        let mut input = Self::parse(value, usage)?;
        input.arc_length = true;
        Ok(input)
    }
}

pub(super) fn resolve<'a>(
    document: &'a Document,
    target: ObjectId,
    subcurves: &[SubcurveInput],
    usage: &'static str,
) -> Result<Vec<Input<'a>>, CommandError> {
    let mut inputs = document
        .selected_objects()
        .filter(|o| o.id() != target)
        .filter(|o| {
            o.geometry().curve_ref().is_some() || matches!(o.geometry(), Geometry::Point(_))
        })
        .map(|o| Input {
            object: o.id(),
            geometry: Cow::Borrowed(o.geometry()),
            temporary: false,
        })
        .collect::<Vec<_>>();
    for input in subcurves {
        if input.object == target || !document.is_object_selectable(input.object) {
            return Err(CommandError::Usage(usage));
        }
        let curve = document
            .object(input.object)
            .and_then(|o| o.geometry().curve_ref())
            .ok_or(CommandError::Usage(usage))?;
        let [start, end] = input.parameters;
        let piece = if input.arc_length {
            curve
                .to_owned()
                .try_subcurve_at_arc_length(start, end, document.tolerance())?
                .ok_or(CommandError::Usage(
                    "Subcurve length exceeds the available curve",
                ))?
        } else {
            curve.to_owned().try_subcurve(start, end)?
        };
        inputs.push(Input {
            object: input.object,
            geometry: Cow::Owned(Geometry::from(piece)),
            temporary: true,
        });
    }
    if subcurves.is_empty() {
        return Ok(inputs);
    }
    let order = document
        .objects()
        .enumerate()
        .map(|(i, o)| (o.id(), i))
        .collect::<BTreeMap<_, _>>();
    inputs.sort_by_key(|input| order[&input.object]);
    Ok(inputs)
}

/// All pieces from a source group share one independent output group.
pub(super) fn copy_outputs(
    document: &mut Document,
    staged: Vec<(Option<ObjectId>, Geometry)>,
) -> Result<Vec<ObjectId>, CommandError> {
    let mut outputs = Vec::new();
    let mut pieces = Vec::new();
    for (source, geometry) in staged {
        if let Some(source) = source {
            let text = document
                .object(source)
                .ok_or(DocumentError::ObjectNotFound(source))?
                .geometry_user_text()
                .clone();
            pieces.push((source, geometry, text));
        } else {
            outputs.extend(
                document.copy_object_pieces_with_metadata_into_source_groups(std::mem::take(
                    &mut pieces,
                ))?,
            );
            outputs.push(document.add_geometry(geometry)?);
        }
    }
    outputs.extend(document.copy_object_pieces_with_metadata_into_source_groups(pieces)?);
    let mut group_map = BTreeMap::new();
    for &id in &outputs {
        let memberships = document.object(id).unwrap().group_ids().to_vec();
        let mut copies = Vec::new();
        for group in memberships {
            let new_group = if let Some(&copy) = group_map.get(&group) {
                copy
            } else {
                let copy = document.add_empty_group(Some(document.next_unused_group_name()))?;
                group_map.insert(group, copy);
                copy
            };
            copies.push(new_group);
        }
        document.set_object_group_memberships(id, copies)?;
    }
    Ok(outputs)
}
