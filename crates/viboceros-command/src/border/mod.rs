//! Exact joined borders, independent of selection and registry transactions.
use crate::*;
mod assemble;
mod face;
#[cfg(test)]
mod tests;
pub(crate) use assemble::assemble;
use assemble::surface_components;
pub(super) use face::DuplicateFaceBorderCommand;

const DUPLICATE_BORDER_USAGE: &str =
    "DupBorder [Faces=All|Faces=0,2,...] [OutputLayer=Current|Input]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DuplicateBorderOutputLayer {
    Current,
    Input,
}

struct StagedDuplicateBorder {
    loops: Vec<Geometry>,
    attributes: ObjectAttributes,
    groups: Vec<GroupId>,
}

pub(super) struct DuplicateBorderCommand;

impl Command for DuplicateBorderCommand {
    fn name(&self) -> &'static str {
        "DupBorder"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["DuplicateBorder"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        // Typed face indices stand in for Rhino's preselected face subobjects.
        if arguments.iter().any(|arg| {
            let name = arg.split_once('=').map_or(*arg, |(name, _)| name);
            option_name_eq(name, "Faces") || option_name_eq(name, "FaceIndices")
        }) {
            return face::run(document, arguments, true);
        }
        let output_layer = parse_duplicate_border_arguments(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| (object.geometry(), object.attributes(), object.group_ids()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let current_layer = document.current_layer_id();
        let mut staged = Vec::new();
        let mut border_count = 0_usize;
        for (geometry, input_attributes, input_groups) in selected {
            let loops: Vec<Geometry> = match geometry {
                Geometry::NurbsSurface(surface) => surface_components(surface)?
                    .into_iter()
                    .map(|curves| assemble(curves, document.tolerance()))
                    .collect::<Result<_, _>>()?,
                Geometry::Brep(brep) => (if brep.faces().len() == 1 {
                    brep.face_boundary_curve_components(0)?
                } else {
                    brep.naked_boundary_curve_components()?
                })
                .into_iter()
                .map(|curves| assemble(curves, document.tolerance()))
                .collect::<Result<_, _>>()?,
                Geometry::Mesh(mesh) => assemble::mesh_boundaries(mesh, document.tolerance())?,
                Geometry::Point(_)
                | Geometry::PointCloud(_)
                | Geometry::Line(_)
                | Geometry::Circle(_)
                | Geometry::Arc(_)
                | Geometry::Ellipse(_)
                | Geometry::Polyline(_)
                | Geometry::NurbsCurve(_)
                | Geometry::PolyCurve(_) => {
                    return Err(CommandError::UnsupportedDuplicateBorderGeometry);
                }
            };
            if loops.is_empty() {
                continue;
            }
            let (attributes, groups) = match output_layer {
                DuplicateBorderOutputLayer::Current => {
                    (ObjectAttributes::on_layer(current_layer), vec![])
                }
                DuplicateBorderOutputLayer::Input => {
                    (input_attributes.clone(), input_groups.to_vec())
                }
            };
            border_count = border_count
                .checked_add(loops.len())
                .filter(|count| *count <= MAX_SPAN_OUTPUT_OBJECTS)
                .ok_or_else(|| too_many_span_outputs("DupBorder"))?;
            staged.push(StagedDuplicateBorder {
                loops,
                attributes,
                groups,
            });
        }
        if staged.is_empty() {
            return Err(CommandError::NoDuplicateBorders);
        }

        let source_count = staged.len();
        let mut output_ids = Vec::with_capacity(border_count);
        for source in staged {
            for geometry in source.loops {
                let id =
                    document.add_geometry_with_attributes(geometry, source.attributes.clone())?;
                document.set_object_group_memberships(id, source.groups.iter().copied())?;
                output_ids.push(id);
            }
        }
        // Input-layer copies may share groups with the retained sources.
        // Selecting the new IDs must not expand those groups back to sources.
        document.select_objects_direct(output_ids, SelectionMode::Replace)?;
        Ok(format!(
            "Duplicated {border_count} border curve(s) in {border_count} border(s) from {source_count} object(s)"
        ))
    }
}

fn parse_duplicate_border_arguments(
    arguments: &[&str],
) -> Result<DuplicateBorderOutputLayer, CommandError> {
    if arguments.is_empty() {
        return Ok(DuplicateBorderOutputLayer::Current);
    }
    let (name, value, consumed) = if let Some((name, value)) = arguments[0].split_once('=') {
        (name, value, 1)
    } else {
        let value = arguments
            .get(1)
            .ok_or(CommandError::Usage(DUPLICATE_BORDER_USAGE))?;
        (arguments[0], *value, 2)
    };
    require_consumed(arguments, consumed, DUPLICATE_BORDER_USAGE)?;
    if !option_name_eq(name, "OutputLayer") {
        return Err(CommandError::Usage(DUPLICATE_BORDER_USAGE));
    }
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("Current") {
        Ok(DuplicateBorderOutputLayer::Current)
    } else if value.eq_ignore_ascii_case("Input") {
        Ok(DuplicateBorderOutputLayer::Input)
    } else {
        Err(CommandError::Usage(DUPLICATE_BORDER_USAGE))
    }
}
