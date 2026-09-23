//! Intersect two explicit object sets with per-set output-layer policy.

use super::*;

pub(super) struct IntersectTwoSetsCommand;

const INTERSECT_TWO_SETS_USAGE: &str = "IntersectTwoSets first-id[,id...]|Selected second-id[,id...]|Selected [OutputLayer=Current|FirstSet|SecondSet]";

#[derive(Clone, Copy)]
enum IntersectionOutputLayer {
    Current,
    FirstSet,
    SecondSet,
}

fn parse_intersect_ids(value: &str, document: &Document) -> Result<Vec<ObjectId>, CommandError> {
    if value.eq_ignore_ascii_case("Selected") {
        let ids = document.selected_object_ids().collect::<Vec<_>>();
        return if ids.is_empty() {
            Err(CommandError::NoObjectsSelected)
        } else {
            Ok(ids)
        };
    }
    let mut seen = BTreeSet::new();
    let mut ids = Vec::new();
    for part in value.split(',') {
        let id = part
            .parse::<ObjectId>()
            .map_err(|_| CommandError::Usage(INTERSECT_TWO_SETS_USAGE))?;
        if seen.insert(id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return Err(CommandError::Usage(INTERSECT_TWO_SETS_USAGE));
    }
    Ok(ids)
}

impl Command for IntersectTwoSetsCommand {
    fn name(&self) -> &'static str {
        "IntersectTwoSets"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let [first_arg, second_arg, options @ ..] = arguments else {
            return Err(CommandError::Usage(INTERSECT_TWO_SETS_USAGE));
        };
        let mut output_layer = IntersectionOutputLayer::Current;
        if options.len() > 1 {
            return Err(CommandError::Usage(INTERSECT_TWO_SETS_USAGE));
        }
        if let Some(option) = options.first() {
            let Some((name, value)) = option.trim_start_matches('_').split_once('=') else {
                return Err(CommandError::Usage(INTERSECT_TWO_SETS_USAGE));
            };
            if !name.eq_ignore_ascii_case("OutputLayer") {
                return Err(CommandError::Usage(INTERSECT_TWO_SETS_USAGE));
            }
            output_layer = match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                "current" => IntersectionOutputLayer::Current,
                "firstset" => IntersectionOutputLayer::FirstSet,
                "secondset" => IntersectionOutputLayer::SecondSet,
                _ => return Err(CommandError::Usage(INTERSECT_TWO_SETS_USAGE)),
            };
        }
        let first_ids = parse_intersect_ids(first_arg, document)?;
        let second_ids = parse_intersect_ids(second_arg, document)?;
        let second_set = second_ids.iter().copied().collect::<BTreeSet<_>>();
        let overlap = first_ids
            .iter()
            .filter(|id| second_set.contains(id))
            .count();
        let pair_count = first_ids
            .len()
            .checked_mul(second_ids.len())
            .and_then(|count| count.checked_sub(overlap))
            .ok_or(CommandError::TooManyIntersectPairs {
                maximum: MAX_INTERSECT_PAIRS,
            })?;
        if pair_count > MAX_INTERSECT_PAIRS {
            return Err(CommandError::TooManyIntersectPairs {
                maximum: MAX_INTERSECT_PAIRS,
            });
        }
        let inputs = |ids: &[ObjectId]| -> Result<Vec<_>, CommandError> {
            ids.iter()
                .map(|&id| {
                    let object = document
                        .object(id)
                        .ok_or(DocumentError::ObjectNotFound(id))?;
                    if !document.is_object_selectable(id) {
                        return Err(CommandError::Usage(INTERSECT_TWO_SETS_USAGE));
                    }
                    Ok((
                        id,
                        object.attributes().layer_id(),
                        intersect_input(object.geometry())?,
                    ))
                })
                .collect()
        };
        let first = inputs(&first_ids)?;
        let second = inputs(&second_ids)?;
        let mut output = Vec::new();
        for (first_id, first_layer, first_input) in &first {
            for (second_id, second_layer, second_input) in &second {
                if first_id == second_id {
                    continue;
                }
                let layer = match output_layer {
                    IntersectionOutputLayer::Current => document.current_layer_id(),
                    IntersectionOutputLayer::FirstSet => *first_layer,
                    IntersectionOutputLayer::SecondSet => *second_layer,
                };
                let mut pair_points = Vec::new();
                for geometry in intersect_pair(first_input, second_input, document.tolerance())? {
                    if let Geometry::Point(point) = &geometry {
                        if pair_points.iter().any(|existing| {
                            model_points_near(*existing, *point, document.tolerance())
                        }) {
                            continue;
                        }
                        pair_points.push(*point);
                    }
                    if output.len() == MAX_INTERSECT_OUTPUTS {
                        return Err(CommandError::TooManyIntersectOutputs {
                            maximum: MAX_INTERSECT_OUTPUTS,
                        });
                    }
                    output.push((geometry, layer));
                }
            }
        }
        let mut output_ids = Vec::with_capacity(output.len());
        for (geometry, layer) in output {
            output_ids.push(
                document
                    .add_geometry_with_attributes(geometry, ObjectAttributes::on_layer(layer))?,
            );
        }
        document.select_objects_direct(output_ids.iter().copied(), SelectionMode::Replace)?;
        Ok(format!(
            "Created {} intersection object(s) from {pair_count} object pair(s)",
            output_ids.len()
        ))
    }
}
