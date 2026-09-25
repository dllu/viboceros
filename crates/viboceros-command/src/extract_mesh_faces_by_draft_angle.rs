//! Extract faces by their normal angle to the active view direction.

use super::*;

const USAGE: &str = "ExtractMeshFacesByDraftAngle StartAngle=degrees EndAngle=degrees ViewDirection=x,y,z [MakeCopy=Yes|No] [BorderOnly=Yes|No]";

#[derive(Clone, Copy)]
struct Options {
    start: Real,
    end: Real,
    viewward: Vector3,
    make_copy: bool,
    border_only: bool,
}

pub(super) struct ExtractMeshFacesByDraftAngleCommand;

impl Command for ExtractMeshFacesByDraftAngleCommand {
    fn name(&self) -> &'static str {
        "ExtractMeshFacesByDraftAngle"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        mesh_face_filter::extract_selected_mesh_faces(
            document,
            "ExtractMeshFacesByDraftAngle",
            mesh_face_filter::FilterOutputOptions {
                make_copy: options.make_copy,
                border_only: options.border_only,
            },
            CommandError::UnsupportedExtractMeshFacesByDraftAngleGeometry,
            CommandError::NoMeshFacesInDraftAngleRange,
            CommandError::NoMeshFaceDraftAngleBorders,
            |mesh| mesh.faces_by_draft_angle(options.viewward, options.start, options.end),
        )
    }
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let mut start = None;
    let mut end = None;
    let mut viewward = None;
    let mut make_copy = None;
    let mut border_only = None;
    for &argument in arguments {
        let Some((key, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(USAGE));
        };
        if key.eq_ignore_ascii_case("StartAngle") && start.is_none() {
            start = Some(parse_angle(value)?);
        } else if key.eq_ignore_ascii_case("EndAngle") && end.is_none() {
            end = Some(parse_angle(value)?);
        } else if key.eq_ignore_ascii_case("ViewDirection") && viewward.is_none() {
            viewward = Some(parse_direction(value)?);
        } else if key.eq_ignore_ascii_case("MakeCopy") && make_copy.is_none() {
            make_copy = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("BorderOnly") && border_only.is_none() {
            border_only = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    let start = start.ok_or(CommandError::Usage(USAGE))?;
    let end = end.ok_or(CommandError::Usage(USAGE))?;
    if start > end {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(Options {
        start,
        end,
        viewward: viewward.ok_or(CommandError::Usage(USAGE))?,
        make_copy: make_copy.unwrap_or(false),
        border_only: border_only.unwrap_or(false),
    })
}

fn parse_angle(value: &str) -> Result<Real, CommandError> {
    value
        .parse::<Real>()
        .ok()
        .filter(|angle| angle.is_finite() && (0.0..=180.0).contains(angle))
        .ok_or(CommandError::Usage(USAGE))
}

fn parse_direction(value: &str) -> Result<Vector3, CommandError> {
    let coordinates = value.split(',').collect::<Vec<_>>();
    let [x, y, z] = coordinates.as_slice() else {
        return Err(CommandError::Usage(USAGE));
    };
    let parse = |coordinate: &str| {
        coordinate
            .parse::<Real>()
            .map_err(|_| CommandError::Usage(USAGE))
    };
    let vector = Vector3::try_new(parse(x)?, parse(y)?, parse(z)?)
        .map_err(|_| CommandError::Usage(USAGE))?;
    vector
        .normalized_nonzero()
        .map_err(|_| CommandError::Usage(USAGE))?;
    Ok(vector)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::MeshFace;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn selected_mesh(document: &mut Document) -> ObjectId {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 3.0, 1.0),
            ],
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([3, 5, 4]),
                MeshFace::Triangle([6, 7, 8]),
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
    fn extracts_view_facing_face_and_undoes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=0 ViewDirection=0,0,1",
                )
                .unwrap(),
            "Extracted 1 mesh face(s) from 1 mesh(es); source faces removed"
        );
        let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(remainder.face_count(), 2);
        document.undo().unwrap();
        let Geometry::Mesh(restored) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(restored.face_count(), 3);
    }

    #[test]
    fn copy_and_border_modes_preserve_source_and_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let group = document.add_group(None, [source]).unwrap();
        registry.execute(
            &mut document,
            "ExtractMeshFacesByDraftAngle StartAngle=90 EndAngle=90 ViewDirection=0,0,1 MakeCopy=Yes",
        ).unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].group_ids().contains(&group));
        assert!(matches!(selected[0].geometry(), Geometry::Mesh(_)));
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry.execute(
            &mut document,
            "ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=0 ViewDirection=0,0,1 BorderOnly=Yes",
        ).unwrap();
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
    fn invalid_arguments_and_nonmesh_selection_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let before = document.undo_label().map(str::to_owned);
        for input in [
            "ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=90",
            "ExtractMeshFacesByDraftAngle StartAngle=90 EndAngle=0 ViewDirection=0,0,1",
            "ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=181 ViewDirection=0,0,1",
            "ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=90 ViewDirection=0,0,0",
            "ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=90 ViewDirection=0,0,1 StartAngle=1",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            assert_eq!(document.objects().count(), 1);
            assert_eq!(document.undo_label(), before.as_deref());
        }
        let other = document
            .add_geometry(Geometry::Point(point(9.0, 0.0, 0.0)))
            .unwrap();
        document
            .select_objects_direct([source, other], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(
                &mut document,
                "ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=90 ViewDirection=0,0,1"
            ),
            Err(CommandError::UnsupportedExtractMeshFacesByDraftAngleGeometry)
        ));
        assert_eq!(document.objects().count(), 2);
    }
}
