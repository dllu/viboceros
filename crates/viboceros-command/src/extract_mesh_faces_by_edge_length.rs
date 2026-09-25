//! Extract mesh faces having a boundary edge below or above a length limit.

use super::*;

const USAGE: &str = "ExtractMeshFacesByEdgeLength EdgeLength=value [Select=Shorter|Longer] [MakeCopy=Yes|No] [BorderOnly=Yes|No]";

#[derive(Clone, Copy)]
enum Comparison {
    Shorter,
    Longer,
}

#[derive(Clone, Copy)]
struct Options {
    length: Real,
    comparison: Comparison,
    make_copy: bool,
    border_only: bool,
}

pub(super) struct ExtractMeshFacesByEdgeLengthCommand;

impl Command for ExtractMeshFacesByEdgeLengthCommand {
    fn name(&self) -> &'static str {
        "ExtractMeshFacesByEdgeLength"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        mesh_face_filter::extract_filtered_mesh_faces(
            document,
            "ExtractMeshFacesByEdgeLength",
            mesh_face_filter::FilterOutputOptions {
                make_copy: options.make_copy,
                border_only: options.border_only,
            },
            CommandError::UnsupportedExtractMeshFacesByEdgeLengthGeometry,
            CommandError::NoMeshFacesInEdgeLengthRange,
            CommandError::NoMeshFaceEdgeLengthBorders,
            |mesh, index| {
                let (shortest, longest) = mesh.face_edge_length_range(index)?;
                Ok(match options.comparison {
                    Comparison::Shorter => shortest < options.length,
                    Comparison::Longer => longest > options.length,
                })
            },
        )
    }
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let mut length = None;
    let mut comparison = None;
    let mut make_copy = None;
    let mut border_only = None;
    for &argument in arguments {
        let Some((key, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(USAGE));
        };
        if key.eq_ignore_ascii_case("EdgeLength") && length.is_none() {
            length = Some(
                value
                    .parse::<Real>()
                    .ok()
                    .filter(|length| length.is_finite() && *length > 0.0)
                    .ok_or(CommandError::Usage(USAGE))?,
            );
        } else if key.eq_ignore_ascii_case("Select") && comparison.is_none() {
            comparison = Some(if value.eq_ignore_ascii_case("Shorter") {
                Comparison::Shorter
            } else if value.eq_ignore_ascii_case("Longer") {
                Comparison::Longer
            } else {
                return Err(CommandError::Usage(USAGE));
            });
        } else if key.eq_ignore_ascii_case("MakeCopy") && make_copy.is_none() {
            make_copy = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("BorderOnly") && border_only.is_none() {
            border_only = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    Ok(Options {
        length: length.ok_or(CommandError::Usage(USAGE))?,
        comparison: comparison.unwrap_or(Comparison::Shorter),
        make_copy: make_copy.unwrap_or(false),
        border_only: border_only.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::MeshFace;

    fn selected_mesh(document: &mut Document) -> ObjectId {
        let point = |x, y| Point3::try_new(x, y, 0.0).unwrap();
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(2.0, 2.0),
                point(0.0, 2.0),
                point(3.0, 0.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([1, 4, 2])],
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
    fn shorter_mode_uses_any_boundary_edge_and_is_strict() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let before = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "ExtractMeshFacesByEdgeLength EdgeLength=1"),
            Err(CommandError::NoMeshFacesInEdgeLengthRange)
        ));
        assert_eq!(document.undo_label(), before.as_deref());
        assert_eq!(
            registry
                .execute(&mut document, "ExtractMeshFacesByEdgeLength EdgeLength=2")
                .unwrap(),
            "Extracted 1 mesh face(s) from 1 mesh(es); source faces removed"
        );
        let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert!(remainder.faces()[0].is_quad());
        assert_eq!(document.selected_objects().count(), 1);
        document.undo().unwrap();
        let Geometry::Mesh(restored) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(restored.face_count(), 2);
    }

    #[test]
    fn longer_mode_ignores_the_quad_diagonal_and_can_copy() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let group = document.add_group(None, [source]).unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "ExtractMeshFacesByEdgeLength EdgeLength=2.1 Select=Longer MakeCopy=Yes",
            )
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        let Geometry::Mesh(extracted) = selected[0].geometry() else {
            panic!("mesh expected")
        };
        assert!(extracted.faces()[0].is_triangle());
        assert!(selected[0].group_ids().contains(&group));
        let Geometry::Mesh(original) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(original.face_count(), 2);
    }

    #[test]
    fn invalid_options_and_late_nonmesh_leave_document_unchanged() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let other = document
            .add_geometry(Geometry::Point(Point3::try_new(10.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document
            .select_objects_direct([source, other], SelectionMode::Replace)
            .unwrap();
        let before = document.undo_label().map(str::to_owned);
        for input in [
            "ExtractMeshFacesByEdgeLength",
            "ExtractMeshFacesByEdgeLength EdgeLength=0",
            "ExtractMeshFacesByEdgeLength EdgeLength=1 Select=Other",
            "ExtractMeshFacesByEdgeLength EdgeLength=1 EdgeLength=2",
            "ExtractMeshFacesByEdgeLength EdgeLength=2",
        ] {
            assert!(registry.execute(&mut document, input).is_err());
            assert_eq!(document.objects().count(), 2);
            assert_eq!(document.undo_label(), before.as_deref());
        }
    }
}
