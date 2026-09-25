//! Extract one mesh region bounded by naked, unwelded, or nonmanifold edges.

use super::*;
use viboceros_geometry::MeshPartBoundary;

const USAGE: &str = "ExtractMeshPart Face=index [ExtractWholeDisjointParts=Yes|No] [ExtractToNonManifoldEdges=Yes|No] [MakeCopy=Yes|No] [BorderOnly=Yes|No]";

#[derive(Clone, Copy)]
struct Options {
    face: usize,
    boundary: MeshPartBoundary,
    make_copy: bool,
    border_only: bool,
}

pub(super) struct ExtractMeshPartCommand;

impl Command for ExtractMeshPartCommand {
    fn name(&self) -> &'static str {
        "ExtractMeshPart"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        mesh_face_filter::extract_selected_mesh_faces(
            document,
            "ExtractMeshPart",
            mesh_face_filter::FilterOutputOptions {
                make_copy: options.make_copy,
                border_only: options.border_only,
            },
            CommandError::UnsupportedExtractMeshPartGeometry,
            CommandError::NoMeshPartFaces,
            CommandError::NoMeshPartBorders,
            |mesh| mesh.mesh_part_faces(options.face, options.boundary),
        )
    }
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let mut face = None;
    let mut whole_disjoint = None;
    let mut to_nonmanifold = None;
    let mut make_copy = None;
    let mut border_only = None;
    for &argument in arguments {
        let Some((key, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(USAGE));
        };
        if key.eq_ignore_ascii_case("Face") && face.is_none() {
            face = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(USAGE))?,
            );
        } else if key.eq_ignore_ascii_case("ExtractWholeDisjointParts") && whole_disjoint.is_none()
        {
            whole_disjoint = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("ExtractToNonManifoldEdges") && to_nonmanifold.is_none()
        {
            to_nonmanifold = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("MakeCopy") && make_copy.is_none() {
            make_copy = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("BorderOnly") && border_only.is_none() {
            border_only = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    let boundary = if whole_disjoint.unwrap_or(false) {
        MeshPartBoundary::Naked
    } else if to_nonmanifold.unwrap_or(true) {
        MeshPartBoundary::UnweldedAndNonManifold
    } else {
        MeshPartBoundary::Unwelded
    };
    Ok(Options {
        face: face.ok_or(CommandError::Usage(USAGE))?,
        boundary,
        make_copy: make_copy.unwrap_or(false),
        border_only: border_only.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::MeshFace;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn selected_mesh(document: &mut Document) -> ObjectId {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(0.0, 1.0),
                point(1.0, 1.0),
                point(1.0, 0.0),
                point(1.0, 1.0),
                point(2.0, 0.0),
            ],
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([1, 3, 2]),
                MeshFace::Triangle([4, 6, 5]),
            ],
            document.tolerance(),
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        id
    }

    #[test]
    fn extracts_to_unwelded_edge_and_undoes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        assert_eq!(
            registry
                .execute(&mut document, "ExtractMeshPart Face=0")
                .unwrap(),
            "Extracted 2 mesh face(s) from 1 mesh(es); source faces removed"
        );
        let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(remainder.face_count(), 1);
        assert_eq!(document.selected_objects().count(), 1);
        document.undo().unwrap();
        let Geometry::Mesh(restored) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(restored.face_count(), 3);
    }

    #[test]
    fn whole_disjoint_copy_and_border_modes_preserve_source_and_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let group = document.add_group(None, [source]).unwrap();
        registry
            .execute(
                &mut document,
                "ExtractMeshPart Face=0 ExtractWholeDisjointParts=Yes MakeCopy=Yes",
            )
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].group_ids().contains(&group));
        let Geometry::Mesh(extracted) = selected[0].geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(extracted.face_count(), 3);
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "ExtractMeshPart Face=0 BorderOnly=Yes")
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].group_ids().contains(&group));
        assert!(matches!(selected[0].geometry(), Geometry::Polyline(_)));
        let Geometry::Mesh(original) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(original.face_count(), 3);
    }

    #[test]
    fn invalid_inputs_and_late_nonmesh_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let before = document.undo_label().map(str::to_owned);
        for input in [
            "ExtractMeshPart",
            "ExtractMeshPart Face=3",
            "ExtractMeshPart Face=-1",
            "ExtractMeshPart Face=0 ExtractToNonManifoldEdges=Maybe",
            "ExtractMeshPart Face=0 Face=1",
        ] {
            assert!(registry.execute(&mut document, input).is_err());
            assert_eq!(document.objects().count(), 1);
            assert_eq!(document.undo_label(), before.as_deref());
        }
        let other = document
            .add_geometry(Geometry::Point(point(9.0, 0.0)))
            .unwrap();
        document
            .select_objects_direct([source, other], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "ExtractMeshPart Face=0"),
            Err(CommandError::UnsupportedExtractMeshPartGeometry)
        ));
        assert_eq!(document.objects().count(), 2);
    }

    #[test]
    fn nonmanifold_boundary_option_controls_traversal() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(0.0, 1.0),
                point(0.0, -1.0),
                Point3::try_new(0.0, 0.0, 1.0).unwrap(),
            ],
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([1, 0, 3]),
                MeshFace::Triangle([0, 1, 4]),
            ],
            document.tolerance(),
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "ExtractMeshPart Face=0 MakeCopy=Yes")
            .unwrap();
        let Geometry::Mesh(default_part) = document.selected_objects().next().unwrap().geometry()
        else {
            panic!("mesh expected")
        };
        assert_eq!(default_part.face_count(), 1);
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "ExtractMeshPart Face=0 ExtractToNonManifoldEdges=No MakeCopy=Yes",
            )
            .unwrap();
        let Geometry::Mesh(extended_part) = document.selected_objects().next().unwrap().geometry()
        else {
            panic!("mesh expected")
        };
        assert_eq!(extended_part.face_count(), 3);
    }
}
