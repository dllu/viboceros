//! Face picking and per-face border extraction.
use super::*;

const DUPLICATE_FACE_BORDER_USAGE: &str =
    "DupFaceBorder (point|Faces=All|Faces=0,2,...) [OutputLayer=Current|Input]";

#[derive(Clone, Debug, PartialEq)]
struct DuplicateFaceBorderOptions {
    selection: SurfaceFaceSelection,
    output_layer: DuplicateBorderOutputLayer,
}

pub(crate) struct DuplicateFaceBorderCommand;

impl Command for DuplicateFaceBorderCommand {
    fn name(&self) -> &'static str {
        "DupFaceBorder"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["DuplicateFaceBorder"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        run(document, arguments, false)
    }
}

pub(super) fn run(
    document: &mut Document,
    arguments: &[&str],
    copy_input_attributes: bool,
) -> Result<String, CommandError> {
    let options = parse_duplicate_face_border_arguments(arguments)?;
    let sources = document
        .selected_objects()
        .map(|object| {
            if !matches!(
                object.geometry(),
                Geometry::NurbsSurface(_) | Geometry::Brep(_)
            ) {
                return Err(CommandError::UnsupportedDuplicateFaceBorderGeometry);
            }
            Ok(SurfaceFaceSource {
                id: object.id(),
                geometry: object.geometry().clone(),
                attributes: object.attributes().clone(),
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    if sources.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    let selections = selected_surface_faces(
        &sources,
        &options.selection,
        document.tolerance(),
        SurfaceFaceCommand::DuplicateFaceBorder,
    )?;
    let current_layer = document.current_layer_id();
    let mut staged = Vec::new();
    let mut output_count = 0_usize;
    let mut border_count = 0_usize;
    for (source_index, faces) in selections {
        let source = &sources[source_index];
        let mut source_borders = Vec::new();
        for face in faces {
            let components = match &source.geometry {
                Geometry::NurbsSurface(surface) => {
                    debug_assert_eq!(face, 0);
                    surface_components(surface)?
                }
                Geometry::Brep(brep) => brep.face_boundary_curve_components(face)?,
                _ => unreachable!("surface-face sources were validated above"),
            };
            for component in components {
                let geometry = assemble(component, document.tolerance())?;
                output_count = output_count
                    .checked_add(1)
                    .filter(|count| *count <= MAX_SPAN_OUTPUT_OBJECTS)
                    .ok_or_else(|| too_many_span_outputs("DupFaceBorder"))?;
                border_count += 1;
                source_borders.push(geometry);
            }
        }
        if !source_borders.is_empty() {
            let layer = match options.output_layer {
                DuplicateBorderOutputLayer::Current => current_layer,
                DuplicateBorderOutputLayer::Input => source.attributes.layer_id(),
            };
            let (attributes, groups) = if copy_input_attributes
                && options.output_layer == DuplicateBorderOutputLayer::Input
            {
                (
                    source.attributes.clone(),
                    document
                        .object(source.id)
                        .expect("validated source")
                        .group_ids()
                        .to_vec(),
                )
            } else {
                (ObjectAttributes::on_layer(layer), vec![])
            };
            staged.push((attributes, groups, source_borders));
        }
    }
    if staged.is_empty() {
        return Err(CommandError::NoDuplicateFaceBorders);
    }

    let source_count = staged.len();
    let mut output_ids = Vec::with_capacity(output_count);
    for (attributes, groups, borders) in staged {
        for geometry in borders {
            let id = document.add_geometry_with_attributes(geometry, attributes.clone())?;
            document.set_object_group_memberships(id, groups.iter().copied())?;
            output_ids.push(id);
        }
    }
    document.select_objects_direct(output_ids, SelectionMode::Replace)?;
    Ok(format!(
        "Duplicated {output_count} curve object(s) in {border_count} face border(s) from {source_count} object(s)"
    ))
}

fn parse_duplicate_face_border_arguments(
    arguments: &[&str],
) -> Result<DuplicateFaceBorderOptions, CommandError> {
    let mut output_layer = DuplicateBorderOutputLayer::Current;
    let mut face_selection = None;
    let mut output_layer_seen = false;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "OutputLayer")
            || option_name_eq(argument, "Faces")
            || option_name_eq(argument, "FaceIndices")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(DUPLICATE_FACE_BORDER_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        let Some((name, value, consumed)) = option else {
            positional.push(argument);
            index += 1;
            continue;
        };
        if option_name_eq(name, "OutputLayer") && !output_layer_seen {
            let value = value.trim_start_matches('_');
            output_layer = if value.eq_ignore_ascii_case("Current") {
                DuplicateBorderOutputLayer::Current
            } else if value.eq_ignore_ascii_case("Input") {
                DuplicateBorderOutputLayer::Input
            } else {
                return Err(CommandError::Usage(DUPLICATE_FACE_BORDER_USAGE));
            };
            output_layer_seen = true;
        } else if (option_name_eq(name, "Faces") || option_name_eq(name, "FaceIndices"))
            && face_selection.is_none()
        {
            face_selection = Some(parse_surface_face_indices(
                value,
                DUPLICATE_FACE_BORDER_USAGE,
            )?);
        } else {
            return Err(CommandError::Usage(DUPLICATE_FACE_BORDER_USAGE));
        }
        index += consumed;
    }
    let selection = if let Some(face_selection) = face_selection {
        require_consumed(&positional, 0, DUPLICATE_FACE_BORDER_USAGE)?;
        SurfaceFaceSelection::Faces(face_selection)
    } else {
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, DUPLICATE_FACE_BORDER_USAGE)?;
        SurfaceFaceSelection::Point(point)
    };
    Ok(DuplicateFaceBorderOptions {
        selection,
        output_layer,
    })
}
