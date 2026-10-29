//! Extract selected mesh faces with source metadata and grouped history.
use super::selection::*;
use crate::*;

const EXTRACT_MESH_FACES_USAGE: &str = "ExtractMeshFaces (point|Faces=All|Faces=0,2,...|Face=index [Object=selected-uuid]) [MakeCopy=Yes|No]";

#[derive(Clone, Debug, PartialEq)]
struct ExtractMeshFacesOptions {
    selection: FaceSelection,
    make_copy: bool,
}

pub(crate) struct ExtractMeshFacesCommand;

struct ExtractMeshFacesPlan {
    source: ObjectId,
    remainder: Option<TriangleMesh>,
    extracted: TriangleMesh,
    attributes: ObjectAttributes,
    group_ids: Vec<GroupId>,
}

impl Command for ExtractMeshFacesCommand {
    fn name(&self) -> &'static str {
        "ExtractMeshFaces"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_extract_mesh_faces_arguments(arguments)?;
        if options
            .selection
            .object()
            .is_some_and(|id| !document.is_selected(id))
        {
            return Err(CommandError::Usage(EXTRACT_MESH_FACES_USAGE));
        }
        let sources = selected_mesh_face_sources(
            document,
            || CommandError::UnsupportedExtractMeshFacesGeometry,
            options.selection.object(),
        )?;
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let selections = selected_mesh_faces(
            &sources,
            &options.selection,
            FaceEditCommand::ExtractMeshFaces,
        )?;
        if selections.len() > MAX_SPAN_OUTPUT_OBJECTS {
            return Err(too_many_span_outputs("ExtractMeshFaces"));
        }
        let mut extracted_face_count = 0_usize;
        let mut plans = Vec::with_capacity(selections.len());
        for (source_index, face_indices) in selections {
            extracted_face_count = extracted_face_count
                .checked_add(face_indices.len())
                .ok_or_else(|| too_many_span_outputs("ExtractMeshFaces"))?;
            let source = &sources[source_index];
            let (remainder, extracted) = source.mesh.extract_faces(&face_indices)?.into_parts();
            plans.push(ExtractMeshFacesPlan {
                source: source.id,
                remainder,
                extracted,
                attributes: source.attributes.clone(),
                group_ids: source.group_ids.to_vec(),
            });
        }
        let source_count = plans.len();

        if !options.make_copy {
            // Native face arguments stand in for Rhino's sub-object picker;
            // whole-object selection is not part of the stored mesh state.
            document.clear_selection();
            document.replace_object_geometries(plans.iter().map(|plan| {
                (
                    plan.source,
                    Geometry::Mesh(plan.remainder.as_ref().unwrap_or(&plan.extracted).clone()),
                )
            }))?;
        }

        let mut output_ids = Vec::with_capacity(source_count);

        for plan in plans {
            if !options.make_copy && plan.remainder.is_none() {
                output_ids.push(plan.source);
                continue;
            }
            let output = document
                .add_geometry_with_attributes(Geometry::Mesh(plan.extracted), plan.attributes)?;
            output_ids.push(output);
            document.set_object_group_memberships(output, plan.group_ids)?;
        }

        document.select_objects_direct(output_ids, SelectionMode::Replace)?;
        Ok(format!(
            "Extracted {extracted_face_count} mesh face(s) from {source_count} mesh(es); source faces {}",
            if options.make_copy {
                "copied"
            } else {
                "removed"
            }
        ))
    }
}

fn parse_extract_mesh_faces_arguments(
    arguments: &[&str],
) -> Result<ExtractMeshFacesOptions, CommandError> {
    let mut make_copy = false;
    let mut face_selection = None;
    let mut face_index = None;
    let mut object = None;
    let mut make_copy_seen = false;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "MakeCopy")
            || option_name_eq(argument, "Faces")
            || option_name_eq(argument, "FaceIndices")
            || option_name_eq(argument, "Face")
            || option_name_eq(argument, "Object")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(EXTRACT_MESH_FACES_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        let Some((name, value, consumed)) = option else {
            positional.push(argument);
            index += 1;
            continue;
        };
        if option_name_eq(name, "MakeCopy") && !make_copy_seen {
            make_copy = parse_yes_no(value).ok_or(CommandError::Usage(EXTRACT_MESH_FACES_USAGE))?;
            make_copy_seen = true;
        } else if (option_name_eq(name, "Faces") || option_name_eq(name, "FaceIndices"))
            && face_selection.is_none()
        {
            face_selection = Some(parse_surface_face_indices(value, EXTRACT_MESH_FACES_USAGE)?);
        } else if option_name_eq(name, "Face") && face_index.is_none() {
            face_index = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(EXTRACT_MESH_FACES_USAGE))?,
            );
        } else if option_name_eq(name, "Object") && object.is_none() {
            object = Some(
                value
                    .parse::<ObjectId>()
                    .map_err(|_| CommandError::Usage(EXTRACT_MESH_FACES_USAGE))?,
            );
        } else {
            return Err(CommandError::Usage(EXTRACT_MESH_FACES_USAGE));
        }
        index += consumed;
    }
    let selection = if let Some(face) = face_index {
        if face_selection.is_some() || !positional.is_empty() {
            return Err(CommandError::Usage(EXTRACT_MESH_FACES_USAGE));
        }
        if let Some(object) = object {
            FaceSelection::ObjectFace { object, face }
        } else {
            FaceSelection::Faces(SurfaceFaceIndices::Indices(vec![face]))
        }
    } else {
        if object.is_some() {
            return Err(CommandError::Usage(EXTRACT_MESH_FACES_USAGE));
        }
        finish_face_selection(face_selection, &positional, EXTRACT_MESH_FACES_USAGE)?
    };
    Ok(ExtractMeshFacesOptions {
        selection,
        make_copy,
    })
}
