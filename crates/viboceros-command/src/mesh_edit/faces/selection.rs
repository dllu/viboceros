//! Shared mesh face input, source filtering and atomic preflight.
use crate::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum FaceSelection {
    Point(Point3),
    Faces(SurfaceFaceIndices),
    ObjectFace { object: ObjectId, face: usize },
}

impl FaceSelection {
    pub(super) fn object(&self) -> Option<ObjectId> {
        match self {
            Self::ObjectFace { object, .. } => Some(*object),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum FaceEditCommand {
    ExtractMeshFaces,
    DeleteFaces,
}

impl FaceEditCommand {
    pub(super) fn face_index_out_of_range(self, face: usize, face_count: usize) -> CommandError {
        match self {
            Self::ExtractMeshFaces => {
                CommandError::ExtractMeshFacesFaceIndexOutOfRange { face, face_count }
            }
            Self::DeleteFaces => CommandError::DeleteFacesFaceIndexOutOfRange { face, face_count },
        }
    }
}

pub(in crate::mesh_edit) struct MeshFaceSource<'a> {
    pub(in crate::mesh_edit) id: ObjectId,
    pub(in crate::mesh_edit) mesh: &'a TriangleMesh,
    pub(in crate::mesh_edit) attributes: &'a ObjectAttributes,
    pub(in crate::mesh_edit) group_ids: &'a [GroupId],
}

pub(in crate::mesh_edit) fn selected_mesh_face_sources(
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

pub(super) fn selected_mesh_faces(
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

pub(super) fn parse_face_arguments(
    arguments: &[&str],
    usage: &'static str,
) -> Result<FaceSelection, CommandError> {
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
            let value = arguments.get(index + 1).ok_or(CommandError::Usage(usage))?;
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
            face_selection = Some(parse_surface_face_indices(value, usage)?);
        } else if option_name_eq(name, "Face") && face_index.is_none() {
            face_index = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(usage))?,
            );
        } else if option_name_eq(name, "Object") && object.is_none() {
            object = Some(
                value
                    .parse::<ObjectId>()
                    .map_err(|_| CommandError::Usage(usage))?,
            );
        } else {
            return Err(CommandError::Usage(usage));
        }
        index += consumed;
    }
    if let Some(face) = face_index {
        if face_selection.is_some() || !positional.is_empty() {
            return Err(CommandError::Usage(usage));
        }
        Ok(if let Some(object) = object {
            FaceSelection::ObjectFace { object, face }
        } else {
            FaceSelection::Faces(SurfaceFaceIndices::Indices(vec![face]))
        })
    } else {
        if object.is_some() {
            return Err(CommandError::Usage(usage));
        }
        finish_face_selection(face_selection, &positional, usage)
    }
}

pub(super) fn finish_face_selection(
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
