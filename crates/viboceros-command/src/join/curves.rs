//! Existing curve-chain policy, separate from mesh joining.
use super::*;

pub(super) fn run(document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
    require_consumed(arguments, 0, "Join")?;
    let inputs = selected_join_curves(document)?;
    if inputs.len() < 2 {
        return Err(CommandError::NotEnoughCurvesToJoin);
    }
    let curves = inputs
        .iter()
        .map(|input| input.curve.clone())
        .collect::<Vec<_>>();
    let components = join_curves(
        &curves,
        CurveJoinOptions {
            tolerance: document.tolerance().absolute(),
            preserve_direction: false,
            style: viboceros_geometry::CurveJoinStyle::Seeded,
        },
        document.tolerance(),
    )?;
    let replacements = components
        .iter()
        .filter(|component| component.source_indices().len() > 1)
        .map(|component| {
            let source_indices = component.source_indices().to_vec();
            let attributes = inputs[source_indices[0]].attributes.clone();
            (source_indices, component.curve().clone(), attributes)
        })
        .collect::<Vec<_>>();
    if replacements.is_empty() {
        return Err(CommandError::NoJoinableCurves);
    }

    let joined_curve_count = replacements
        .iter()
        .map(|(sources, _, _)| sources.len())
        .sum::<usize>();
    let unchanged = inputs.len() - joined_curve_count;
    let unchanged_ids = components
        .iter()
        .filter(|component| component.source_indices().len() == 1)
        .map(|component| inputs[component.source_indices()[0]].id)
        .collect::<Vec<_>>();
    let mut result_ids = Vec::with_capacity(replacements.len());
    for (sources, curve, attributes) in replacements {
        let groups = document
            .object(inputs[sources[0]].id)
            .expect("join seed object")
            .group_ids()
            .to_vec();
        let id = document.add_geometry_with_attributes(Geometry::from(curve), attributes)?;
        for group in groups {
            document.add_group_members(group, [id])?;
        }
        for source in sources {
            document.delete_object(inputs[source].id)?;
        }
        result_ids.push(id);
    }
    replace_selection(
        document,
        unchanged_ids.into_iter().chain(result_ids.iter().copied()),
    )?;
    Ok(format!(
        "Joined {joined_curve_count} curve(s) into {} curve(s); {unchanged} curve(s) unchanged",
        result_ids.len()
    ))
}

#[derive(Clone)]
struct SelectedJoinCurve {
    id: ObjectId,
    curve: Curve3,
    attributes: ObjectAttributes,
}

fn selected_join_curves(document: &Document) -> Result<Vec<SelectedJoinCurve>, CommandError> {
    let mut inputs = Vec::new();
    for object in document.selected_objects() {
        let curve = object
            .geometry()
            .curve_ref()
            .ok_or(CommandError::UnsupportedJoinGeometry)?
            .to_owned();
        inputs.push(SelectedJoinCurve {
            id: object.id(),
            curve,
            attributes: object.attributes().clone(),
        });
    }
    if inputs.is_empty() {
        Err(CommandError::NoObjectsSelected)
    } else {
        Ok(inputs)
    }
}
