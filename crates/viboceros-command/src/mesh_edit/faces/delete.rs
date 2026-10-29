//! Delete mesh or B-rep faces after complete source preflight.
use super::selection::*;
use crate::*;

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
        let selection = parse_face_arguments(arguments, DELETE_FACES_USAGE)?;
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
