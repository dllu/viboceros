//! Computed face-set extraction with shared attribute and group handling.
use crate::*;

const EXTRACT_NON_MANIFOLD_USAGE: &str =
    "ExtractNonManifoldMeshEdges [ExtractHangingFacesOnly=Yes|No] [MinimumFaceCount=count]";

pub(crate) struct ExtractNonManifoldMeshEdgesCommand;

impl Command for ExtractNonManifoldMeshEdgesCommand {
    fn name(&self) -> &'static str {
        "ExtractNonManifoldMeshEdges"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_extract_non_manifold_arguments(arguments)?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let inputs = stage_selected_mesh_face_extractions(
            document,
            || CommandError::UnsupportedExtractNonManifoldGeometry,
            |mesh| {
                mesh.extract_non_manifold_faces(
                    options.minimum_face_count,
                    options.hanging_faces_only,
                )
            },
        )?;
        let counts = mesh_face_extraction_counts(&inputs);
        if counts.extracted_meshes == 0 {
            return Err(CommandError::NoNonManifoldMeshFaces);
        }
        apply_mesh_face_extractions(document, inputs, counts.extracted_meshes)?;
        Ok(format!(
            "Extracted {} face(s) from {} mesh(es); {} mesh(es) unchanged",
            counts.extracted_faces, counts.extracted_meshes, counts.unchanged_meshes
        ))
    }
}

pub(super) struct MeshFaceExtractionInput {
    pub(super) id: ObjectId,
    pub(super) attributes: ObjectAttributes,
    pub(super) group_ids: Vec<GroupId>,
    pub(super) extraction: Option<(Option<TriangleMesh>, TriangleMesh)>,
}

#[derive(Clone, Copy)]
struct MeshFaceExtractionCounts {
    extracted_meshes: usize,
    unchanged_meshes: usize,
    extracted_faces: usize,
}

pub(super) fn stage_selected_mesh_face_extractions(
    document: &Document,
    unsupported_geometry: impl Fn() -> CommandError,
    mut extract: impl FnMut(&TriangleMesh) -> Result<Option<MeshFaceExtraction>, GeometryError>,
) -> Result<Vec<MeshFaceExtractionInput>, CommandError> {
    document
        .selected_objects()
        .map(|object| {
            let id = object.id();
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(unsupported_geometry());
            };
            let group_ids = object.group_ids().to_vec();
            Ok(MeshFaceExtractionInput {
                id,
                attributes: object.attributes().clone(),
                group_ids,
                extraction: extract(mesh)?.map(MeshFaceExtraction::into_parts),
            })
        })
        .collect()
}

fn mesh_face_extraction_counts(inputs: &[MeshFaceExtractionInput]) -> MeshFaceExtractionCounts {
    let extracted_meshes = inputs
        .iter()
        .filter(|input| input.extraction.is_some())
        .count();
    let extracted_faces = inputs
        .iter()
        .filter_map(|input| input.extraction.as_ref())
        .map(|(_, extracted)| extracted.face_count())
        .sum();
    MeshFaceExtractionCounts {
        extracted_meshes,
        unchanged_meshes: inputs.len() - extracted_meshes,
        extracted_faces,
    }
}

fn apply_mesh_face_extractions(
    document: &mut Document,
    inputs: Vec<MeshFaceExtractionInput>,
    extracted_mesh_count: usize,
) -> Result<(), CommandError> {
    let replacements = inputs
        .iter()
        .filter_map(|input| {
            input.extraction.as_ref().map(|(remainder, extracted)| {
                (
                    input.id,
                    Geometry::Mesh(remainder.as_ref().unwrap_or(extracted).clone()),
                )
            })
        })
        .collect::<Vec<_>>();
    let replaced = document.replace_object_geometries(replacements)?;
    debug_assert_eq!(replaced, extracted_mesh_count);

    let mut output_ids = Vec::with_capacity(inputs.len() + extracted_mesh_count);

    for input in inputs {
        output_ids.push(input.id);
        let Some((remainder, extracted)) = input.extraction else {
            continue;
        };
        if remainder.is_none() {
            continue;
        }
        let id =
            document.add_geometry_with_attributes(Geometry::Mesh(extracted), input.attributes)?;
        output_ids.push(id);
        document.set_object_group_memberships(id, input.group_ids)?;
    }

    replace_selection(document, output_ids)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct ExtractNonManifoldOptions {
    hanging_faces_only: bool,
    minimum_face_count: usize,
}

fn parse_extract_non_manifold_arguments(
    arguments: &[&str],
) -> Result<ExtractNonManifoldOptions, CommandError> {
    let mut options = ExtractNonManifoldOptions {
        hanging_faces_only: false,
        minimum_face_count: 3,
    };
    let mut hanging_seen = false;
    let mut minimum_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(EXTRACT_NON_MANIFOLD_USAGE))?;
            (argument, *value, 2)
        };
        let name = name.trim_start_matches('_');
        if name.eq_ignore_ascii_case("ExtractHangingFacesOnly") && !hanging_seen {
            options.hanging_faces_only =
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRACT_NON_MANIFOLD_USAGE))?;
            hanging_seen = true;
        } else if name.eq_ignore_ascii_case("MinimumFaceCount") && !minimum_seen {
            options.minimum_face_count = value
                .parse::<usize>()
                .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
            if options.minimum_face_count < 3 {
                return Err(GeometryError::InvalidNonManifoldMinimumFaceCount(
                    options.minimum_face_count,
                )
                .into());
            }
            minimum_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRACT_NON_MANIFOLD_USAGE));
        }
        index += consumed;
    }
    Ok(options)
}

pub(crate) struct ExtractDuplicateMeshFacesCommand;

impl Command for ExtractDuplicateMeshFacesCommand {
    fn name(&self) -> &'static str {
        "ExtractDuplicateMeshFaces"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "ExtractDuplicateMeshFaces")?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let inputs = stage_selected_mesh_face_extractions(
            document,
            || CommandError::UnsupportedExtractDuplicateMeshFacesGeometry,
            |mesh| Ok(mesh.extract_duplicate_faces()),
        )?;
        let counts = mesh_face_extraction_counts(&inputs);
        if counts.extracted_meshes == 0 {
            return Err(CommandError::NoDuplicateMeshFaces);
        }
        apply_mesh_face_extractions(document, inputs, counts.extracted_meshes)?;
        Ok(format!(
            "Extracted {} duplicate face(s) from {} mesh(es); {} mesh(es) unchanged",
            counts.extracted_faces, counts.extracted_meshes, counts.unchanged_meshes
        ))
    }
}
