//! Extract mesh faces whose Rhino-style triangle aspect exceeds a limit.

use super::*;

const USAGE: &str =
    "ExtractMeshFacesByAspectRatio AspectRatio=value [MakeCopy=Yes|No] [BorderOnly=Yes|No]";

#[derive(Clone, Copy)]
struct Options {
    minimum: Real,
    make_copy: bool,
    border_only: bool,
}

pub(super) struct ExtractMeshFacesByAspectRatioCommand;

impl Command for ExtractMeshFacesByAspectRatioCommand {
    fn name(&self) -> &'static str {
        "ExtractMeshFacesByAspectRatio"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        mesh_face_filter::extract_filtered_mesh_faces(
            document,
            "ExtractMeshFacesByAspectRatio",
            mesh_face_filter::FilterOutputOptions {
                make_copy: options.make_copy,
                border_only: options.border_only,
            },
            CommandError::UnsupportedExtractMeshFacesByAspectRatioGeometry,
            CommandError::NoMeshFacesAboveAspectRatio,
            CommandError::NoMeshFaceAspectRatioBorders,
            |mesh, index| Ok(mesh.face_aspect_ratio(index)? > options.minimum),
        )
    }
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let mut minimum = None;
    let mut make_copy = None;
    let mut border_only = None;
    for &argument in arguments {
        let Some((key, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(USAGE));
        };
        if key.eq_ignore_ascii_case("AspectRatio") && minimum.is_none() {
            minimum = Some(
                value
                    .parse::<Real>()
                    .ok()
                    .filter(|ratio| ratio.is_finite() && *ratio >= 1.0)
                    .ok_or(CommandError::Usage(USAGE))?,
            );
        } else if key.eq_ignore_ascii_case("MakeCopy") && make_copy.is_none() {
            make_copy = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("BorderOnly") && border_only.is_none() {
            border_only = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    Ok(Options {
        minimum: minimum.ok_or(CommandError::Usage(USAGE))?,
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
                point(1.0, 1.0),
                point(0.0, 1.0),
                point(2.0, 0.0),
                point(12.0, 0.0),
                point(2.0, 1.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([4, 5, 6])],
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
    fn selects_only_faces_strictly_above_the_limit_and_undoes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let before = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(
                &mut document,
                "ExtractMeshFacesByAspectRatio AspectRatio=11"
            ),
            Err(CommandError::NoMeshFacesAboveAspectRatio)
        ));
        assert_eq!(document.undo_label(), before.as_deref());
        assert_eq!(
            registry
                .execute(&mut document, "ExtractMeshFacesByAspectRatio AspectRatio=2")
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
    fn copy_and_border_modes_keep_the_source_and_select_only_results() {
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
                "ExtractMeshFacesByAspectRatio AspectRatio=2 MakeCopy=Yes",
            )
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].group_ids().contains(&group));
        assert!(matches!(selected[0].geometry(), Geometry::Mesh(_)));
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "ExtractMeshFacesByAspectRatio AspectRatio=2 BorderOnly=Yes",
            )
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].group_ids().contains(&group));
        assert!(matches!(selected[0].geometry(), Geometry::Polyline(_)));
        let Geometry::Mesh(original) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(original.face_count(), 2);
    }

    #[test]
    fn invalid_arguments_and_late_nonmesh_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let other = document
            .add_geometry(Geometry::Point(point(20.0, 0.0)))
            .unwrap();
        document
            .select_objects_direct([source, other], SelectionMode::Replace)
            .unwrap();
        let before = document.undo_label().map(str::to_owned);
        for input in [
            "ExtractMeshFacesByAspectRatio",
            "ExtractMeshFacesByAspectRatio AspectRatio=0.5",
            "ExtractMeshFacesByAspectRatio AspectRatio=2 AspectRatio=3",
            "ExtractMeshFacesByAspectRatio AspectRatio=2",
        ] {
            assert!(registry.execute(&mut document, input).is_err());
            assert_eq!(document.objects().count(), 2);
            assert_eq!(document.undo_label(), before.as_deref());
        }
    }
}
