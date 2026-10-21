//! Explicit face extraction and deletion, including source selection.
use crate::*;

const EXTRACT_MESH_FACES_USAGE: &str = "ExtractMeshFaces (point|Faces=All|Faces=0,2,...|Face=index [Object=selected-uuid]) [MakeCopy=Yes|No]";

#[derive(Clone, Debug, PartialEq)]
enum FaceSelection {
    Point(Point3),
    Faces(SurfaceFaceIndices),
    ObjectFace { object: ObjectId, face: usize },
}

impl FaceSelection {
    fn object(&self) -> Option<ObjectId> {
        match self {
            Self::ObjectFace { object, .. } => Some(*object),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum FaceEditCommand {
    ExtractMeshFaces,
    DeleteFaces,
}

impl FaceEditCommand {
    fn face_index_out_of_range(self, face: usize, face_count: usize) -> CommandError {
        match self {
            Self::ExtractMeshFaces => {
                CommandError::ExtractMeshFacesFaceIndexOutOfRange { face, face_count }
            }
            Self::DeleteFaces => CommandError::DeleteFacesFaceIndexOutOfRange { face, face_count },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ExtractMeshFacesOptions {
    selection: FaceSelection,
    make_copy: bool,
}

pub(crate) struct ExtractMeshFacesCommand;

pub(super) struct MeshFaceSource<'a> {
    pub(super) id: ObjectId,
    pub(super) mesh: &'a TriangleMesh,
    pub(super) attributes: &'a ObjectAttributes,
    pub(super) group_ids: &'a [GroupId],
}

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

pub(super) fn selected_mesh_face_sources(
    document: &Document,
    unsupported_geometry: impl Fn() -> CommandError,
    target: Option<ObjectId>,
) -> Result<Vec<MeshFaceSource<'_>>, CommandError> {
    document
        .selected_objects()
        .filter(|object| target.is_none_or(|id| object.id() == id))
        .map(|object| {
            let id = object.id();
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(unsupported_geometry());
            };
            Ok(MeshFaceSource {
                id,
                mesh,
                attributes: object.attributes(),
                group_ids: object.group_ids(),
            })
        })
        .collect()
}

fn selected_mesh_faces(
    sources: &[MeshFaceSource<'_>],
    selection: &FaceSelection,
    command: FaceEditCommand,
) -> Result<Vec<(usize, Vec<usize>)>, CommandError> {
    match selection {
        FaceSelection::ObjectFace { object, face } => {
            let (source_index, source) = sources
                .iter()
                .enumerate()
                .find(|(_, source)| source.id == *object)
                .ok_or(CommandError::NoObjectsSelected)?;
            if *face >= source.mesh.face_count() {
                return Err(command.face_index_out_of_range(*face, source.mesh.face_count()));
            }
            Ok(vec![(source_index, vec![*face])])
        }
        FaceSelection::Faces(selection) => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let face_count = source.mesh.face_count();
                let face_indices = match selection {
                    SurfaceFaceIndices::All => (0..face_count).collect(),
                    SurfaceFaceIndices::Indices(indices) => {
                        if let Some(&face) = indices.iter().find(|&&face| face >= face_count) {
                            return Err(command.face_index_out_of_range(face, face_count));
                        }
                        indices.clone()
                    }
                };
                Ok((source_index, face_indices))
            })
            .collect(),
        FaceSelection::Point(target) => {
            let mut best = None;
            for (source_index, source) in sources.iter().enumerate() {
                for face_index in 0..source.mesh.face_count() {
                    let point = source.mesh.closest_point_on_face(face_index, *target)?;
                    let distance = point.distance_to(*target)?;
                    if best.is_none_or(|(best_distance, _, _)| distance < best_distance) {
                        best = Some((distance, source_index, face_index));
                    }
                }
            }
            let (_, source, face) =
                best.expect("every selected validated mesh has at least one face");
            Ok(vec![(source, vec![face])])
        }
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

const DELETE_FACES_USAGE: &str =
    "DeleteFaces (point|Faces=All|Faces=0,2,...|Face=index [Object=selected-uuid])";

pub(crate) struct DeleteFacesCommand;

struct DeleteFacesSource {
    id: ObjectId,
    geometry: Geometry,
}

struct DeleteFacesPlan {
    source: ObjectId,
    remainder: Option<Geometry>,
}

impl Command for DeleteFacesCommand {
    fn name(&self) -> &'static str {
        "DeleteFaces"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selection = parse_delete_faces_arguments(arguments)?;
        if selection
            .object()
            .is_some_and(|id| !document.is_selected(id))
        {
            return Err(CommandError::Usage(DELETE_FACES_USAGE));
        }
        let sources = document
            .selected_objects()
            .filter(|object| selection.object().is_none_or(|id| object.id() == id))
            .map(|object| {
                if !matches!(object.geometry(), Geometry::Mesh(_) | Geometry::Brep(_)) {
                    return Err(CommandError::UnsupportedDeleteFacesGeometry);
                }
                Ok(DeleteFacesSource {
                    id: object.id(),
                    geometry: object.geometry().clone(),
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let selections = selected_delete_faces(&sources, &selection, document.tolerance())?;
        let deleted_face_count = selections
            .iter()
            .map(|(_, faces)| faces.len())
            .sum::<usize>();
        let plans = selections
            .into_iter()
            .map(|(source_index, face_indices)| {
                let source = &sources[source_index];
                let remainder = match &source.geometry {
                    Geometry::Mesh(mesh) => mesh.delete_faces(&face_indices)?.map(Geometry::Mesh),
                    Geometry::Brep(brep) => {
                        let deleted = face_indices.iter().copied().collect::<BTreeSet<_>>();
                        let remainder_faces = (0..brep.faces().len())
                            .filter(|face| !deleted.contains(face))
                            .collect::<Vec<_>>();
                        if remainder_faces.is_empty() {
                            None
                        } else {
                            Some(Geometry::Brep(
                                brep.sub_brep(&remainder_faces, document.tolerance())?,
                            ))
                        }
                    }
                    _ => unreachable!("DeleteFaces sources were validated above"),
                };
                Ok(DeleteFacesPlan {
                    source: source.id,
                    remainder,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let source_count = plans.len();

        document.clear_selection();
        document.replace_object_geometries(plans.iter().filter_map(|plan| {
            plan.remainder
                .as_ref()
                .map(|remainder| (plan.source, remainder.clone()))
        }))?;
        for plan in plans {
            if plan.remainder.is_none() {
                document.delete_object(plan.source)?;
            }
        }
        document.clear_selection();
        Ok(format!(
            "Deleted {deleted_face_count} face(s) from {source_count} object(s)"
        ))
    }
}

fn selected_delete_faces(
    sources: &[DeleteFacesSource],
    selection: &FaceSelection,
    tolerance: Tolerance,
) -> Result<Vec<(usize, Vec<usize>)>, CommandError> {
    match selection {
        FaceSelection::ObjectFace { object, face } => {
            let (source_index, source) = sources
                .iter()
                .enumerate()
                .find(|(_, source)| source.id == *object)
                .ok_or(CommandError::NoObjectsSelected)?;
            let face_count = match &source.geometry {
                Geometry::Mesh(mesh) => mesh.face_count(),
                Geometry::Brep(brep) => brep.faces().len(),
                _ => unreachable!("DeleteFaces sources were validated above"),
            };
            if *face >= face_count {
                return Err(FaceEditCommand::DeleteFaces.face_index_out_of_range(*face, face_count));
            }
            Ok(vec![(source_index, vec![*face])])
        }
        FaceSelection::Faces(selection) => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let face_count = match &source.geometry {
                    Geometry::Mesh(mesh) => mesh.face_count(),
                    Geometry::Brep(brep) => brep.faces().len(),
                    _ => unreachable!("DeleteFaces sources were validated above"),
                };
                let face_indices = match selection {
                    SurfaceFaceIndices::All => (0..face_count).collect(),
                    SurfaceFaceIndices::Indices(indices) => {
                        if let Some(&face) = indices.iter().find(|&&face| face >= face_count) {
                            return Err(FaceEditCommand::DeleteFaces
                                .face_index_out_of_range(face, face_count));
                        }
                        indices.clone()
                    }
                };
                Ok((source_index, face_indices))
            })
            .collect(),
        FaceSelection::Point(target) => {
            let mut best: Option<(Real, usize, usize)> = None;
            for (source_index, source) in sources.iter().enumerate() {
                let candidate = match &source.geometry {
                    Geometry::Mesh(mesh) => {
                        let mut nearest: Option<(Real, usize)> = None;
                        for face in 0..mesh.face_count() {
                            let point = mesh.closest_point_on_face(face, *target)?;
                            let distance = point.distance_to(*target)?;
                            if nearest.is_none_or(|(best_distance, _)| distance < best_distance) {
                                nearest = Some((distance, face));
                            }
                        }
                        nearest
                    }
                    Geometry::Brep(brep) => brep
                        .closest_face_parameters(*target, tolerance)?
                        .map(|(face, u, v)| {
                            brep.faces()[face]
                                .surface()
                                .evaluate(u, v)
                                .and_then(|point| point.distance_to(*target))
                                .map(|distance| (distance, face))
                        })
                        .transpose()?,
                    _ => unreachable!("DeleteFaces sources were validated above"),
                };
                let Some((distance, face)) = candidate else {
                    continue;
                };
                if best.is_none_or(|(best_distance, _, _)| distance < best_distance) {
                    best = Some((distance, source_index, face));
                }
            }
            let Some((_, source, face)) = best else {
                return Err(CommandError::NoDeleteFaceAtLocation);
            };
            Ok(vec![(source, vec![face])])
        }
    }
}

fn parse_delete_faces_arguments(arguments: &[&str]) -> Result<FaceSelection, CommandError> {
    let mut face_selection = None;
    let mut face_index = None;
    let mut object = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Faces")
            || option_name_eq(argument, "FaceIndices")
            || option_name_eq(argument, "Face")
            || option_name_eq(argument, "Object")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(DELETE_FACES_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        let Some((name, value, consumed)) = option else {
            positional.push(argument);
            index += 1;
            continue;
        };
        if (option_name_eq(name, "Faces") || option_name_eq(name, "FaceIndices"))
            && face_selection.is_none()
        {
            face_selection = Some(parse_surface_face_indices(value, DELETE_FACES_USAGE)?);
        } else if option_name_eq(name, "Face") && face_index.is_none() {
            face_index = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(DELETE_FACES_USAGE))?,
            );
        } else if option_name_eq(name, "Object") && object.is_none() {
            object = Some(
                value
                    .parse::<ObjectId>()
                    .map_err(|_| CommandError::Usage(DELETE_FACES_USAGE))?,
            );
        } else {
            return Err(CommandError::Usage(DELETE_FACES_USAGE));
        }
        index += consumed;
    }
    if let Some(face) = face_index {
        if face_selection.is_some() || !positional.is_empty() {
            return Err(CommandError::Usage(DELETE_FACES_USAGE));
        }
        Ok(if let Some(object) = object {
            FaceSelection::ObjectFace { object, face }
        } else {
            FaceSelection::Faces(SurfaceFaceIndices::Indices(vec![face]))
        })
    } else {
        if object.is_some() {
            return Err(CommandError::Usage(DELETE_FACES_USAGE));
        }
        finish_face_selection(face_selection, &positional, DELETE_FACES_USAGE)
    }
}

fn finish_face_selection(
    face_selection: Option<SurfaceFaceIndices>,
    positional: &[&str],
    usage: &'static str,
) -> Result<FaceSelection, CommandError> {
    if let Some(faces) = face_selection {
        require_consumed(positional, 0, usage)?;
        Ok(FaceSelection::Faces(faces))
    } else {
        let (point, consumed) = parse_point(positional)?;
        require_consumed(positional, consumed, usage)?;
        Ok(FaceSelection::Point(point))
    }
}
