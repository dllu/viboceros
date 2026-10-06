//! Extract the edge-connected region around one stored mesh face.

use super::*;

const USAGE: &str = "ExtractConnectedMeshFaces (Face=index|FacePoint=x,y,z) [Object=selected-uuid] [Angle=degrees] [Compare=Less|Greater] [MakeCopy=Yes|No] [BorderOnly=Yes|No]";

#[derive(Clone, Copy)]
enum FaceSeed {
    Index(usize),
    Point(Point3),
}

#[derive(Clone, Copy)]
struct Options {
    seed: FaceSeed,
    object: Option<ObjectId>,
    angle: Real,
    greater_than: bool,
    make_copy: bool,
    border_only: bool,
}

pub(super) struct ExtractConnectedMeshFacesCommand;

impl Command for ExtractConnectedMeshFacesCommand {
    fn name(&self) -> &'static str {
        "ExtractConnectedMeshFaces"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        let picked = match options.seed {
            FaceSeed::Point(point) => Some(mesh_face_filter::closest_selected_mesh_face(
                document,
                point,
                || CommandError::UnsupportedExtractConnectedMeshFacesGeometry,
            )?),
            FaceSeed::Index(_) => None,
        };
        if let Some(id) = options.object
            && !document.is_selected(id)
        {
            return Err(CommandError::Usage(USAGE));
        }
        mesh_face_filter::extract_selected_mesh_faces(
            document,
            "ExtractConnectedMeshFaces",
            mesh_face_filter::FilterOutputOptions {
                make_copy: options.make_copy,
                border_only: options.border_only,
            },
            mesh_face_filter::FilterErrors {
                unsupported: CommandError::UnsupportedExtractConnectedMeshFacesGeometry,
                no_matches: CommandError::NoConnectedMeshFaces,
                no_borders: CommandError::NoConnectedMeshFaceBorders,
            },
            options.object,
            |id, mesh| {
                let face = if let Some((picked_id, face)) = picked {
                    if id != picked_id {
                        return Ok(Vec::new());
                    }
                    face
                } else if let FaceSeed::Index(face) = options.seed {
                    face
                } else {
                    unreachable!("point seeds are resolved before extraction")
                };
                mesh.rhinocommon_connected_faces_by_angle(face, options.angle, options.greater_than)
            },
        )
    }
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let mut seed = None;
    let mut object = None;
    let mut angle = None;
    let mut greater_than = None;
    let mut make_copy = None;
    let mut border_only = None;
    for &argument in arguments {
        let Some((key, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(USAGE));
        };
        if key.eq_ignore_ascii_case("Face") && seed.is_none() {
            seed = Some(FaceSeed::Index(
                value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(USAGE))?,
            ));
        } else if key.eq_ignore_ascii_case("FacePoint") && seed.is_none() {
            let (point, _) = parse_point(&[value])?;
            seed = Some(FaceSeed::Point(point));
        } else if key.eq_ignore_ascii_case("Object") && object.is_none() {
            object = Some(
                value
                    .parse::<ObjectId>()
                    .map_err(|_| CommandError::Usage(USAGE))?,
            );
        } else if key.eq_ignore_ascii_case("Angle") && angle.is_none() {
            angle = Some(
                value
                    .parse::<Real>()
                    .ok()
                    .filter(|angle| angle.is_finite() && (0.0..=180.0).contains(angle))
                    .ok_or(CommandError::Usage(USAGE))?,
            );
        } else if key.eq_ignore_ascii_case("Compare") && greater_than.is_none() {
            greater_than = Some(if value.eq_ignore_ascii_case("Less") {
                false
            } else if value.eq_ignore_ascii_case("Greater") {
                true
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
    let seed = seed.ok_or(CommandError::Usage(USAGE))?;
    if object.is_some() && matches!(seed, FaceSeed::Point(_)) {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(Options {
        seed,
        object,
        angle: angle.unwrap_or(0.0),
        greater_than: greater_than.unwrap_or(false),
        make_copy: make_copy.unwrap_or(false),
        border_only: border_only.unwrap_or(false),
    })
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
                point(1.0, 1.0, 0.0),
                point(1.0, 1.0, 1.0),
            ],
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([1, 3, 2]),
                MeshFace::Triangle([1, 4, 3]),
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
    fn positive_angle_extracts_planar_region_and_undo_restores_source() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        assert_eq!(
            registry
                .execute(&mut document, "ExtractConnectedMeshFaces Face=0 Angle=0.1")
                .unwrap(),
            "Extracted 2 mesh face(s) from 1 mesh(es); source faces removed"
        );
        let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(remainder.face_count(), 1);
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(matches!(selected[0].geometry(), Geometry::Mesh(_)));
        document.undo().unwrap();
        let Geometry::Mesh(restored) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(restored.face_count(), 3);
    }

    #[test]
    fn zero_angle_copies_the_whole_connected_component() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "ExtractConnectedMeshFaces Face=0 MakeCopy=Yes"
                )
                .unwrap(),
            "Extracted 3 mesh face(s) from 1 mesh(es); source faces copied"
        );
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert_ne!(selected[0].id(), source);
        let Geometry::Mesh(extracted) = selected[0].geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(extracted.face_count(), 3);
        let Geometry::Mesh(original) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(original.face_count(), 3);
    }

    #[test]
    fn face_point_picks_one_mesh_from_a_multiple_mesh_selection() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let first = selected_mesh(&mut document);
        let second_mesh = TriangleMesh::try_new_faces(
            vec![
                point(10.0, 0.0, 0.0),
                point(11.0, 0.0, 0.0),
                point(10.0, 1.0, 0.0),
            ],
            vec![MeshFace::Triangle([0, 1, 2])],
            document.tolerance(),
        )
        .unwrap();
        let second = document.add_geometry(Geometry::Mesh(second_mesh)).unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "ExtractConnectedMeshFaces FacePoint=10.2,0.2,0 MakeCopy=Yes",
            )
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        let Geometry::Mesh(extracted) = selected[0].geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(extracted.face_count(), 1);
        assert_eq!(document.objects().count(), 3);
        let Geometry::Mesh(unchanged) = document.object(first).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(unchanged.face_count(), 3);
    }

    #[test]
    fn object_qualified_face_seed_ignores_other_selected_geometry() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let mesh = selected_mesh(&mut document);
        let point = document
            .add_geometry(Geometry::Point(point(10.0, 0.0, 0.0)))
            .unwrap();
        document
            .select_objects_direct([mesh, point], SelectionMode::Replace)
            .unwrap();
        assert!(
            registry
                .execute(
                    &mut document,
                    &format!("ExtractConnectedMeshFaces FacePoint=0,0,0 Object={mesh}")
                )
                .is_err()
        );
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    &format!("ExtractConnectedMeshFaces Face=0 Object={mesh} MakeCopy=Yes")
                )
                .unwrap(),
            "Extracted 3 mesh face(s) from 1 mesh(es); source faces copied"
        );
        assert!(!document.is_selected(mesh));
        assert!(!document.is_selected(point));
    }

    #[test]
    fn positive_angle_on_nonmanifold_edge_checks_first_other_face() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
                point(0.0, -1.0, 0.0),
            ],
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([1, 0, 3]),
                MeshFace::Triangle([1, 0, 4]),
            ],
            document.tolerance(),
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        for (face, expected) in [(0, 1), (2, 2)] {
            document
                .select_objects_direct([source], SelectionMode::Replace)
                .unwrap();
            registry
                .execute(
                    &mut document,
                    &format!("ExtractConnectedMeshFaces Face={face} Angle=1 MakeCopy=Yes"),
                )
                .unwrap();
            let selected = document.selected_objects().collect::<Vec<_>>();
            assert_eq!(selected.len(), 1);
            let Geometry::Mesh(extracted) = selected[0].geometry() else {
                panic!("mesh expected")
            };
            assert_eq!(extracted.face_count(), expected);
        }
    }

    #[test]
    fn copy_and_border_modes_preserve_source_and_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let group = document.add_group(None, [source]).unwrap();
        registry
            .execute(
                &mut document,
                "ExtractConnectedMeshFaces Face=1 Angle=90 Compare=Greater MakeCopy=Yes",
            )
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].group_ids().contains(&group));
        let Geometry::Mesh(extracted) = selected[0].geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(extracted.face_count(), 2);
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "ExtractConnectedMeshFaces Face=0 BorderOnly=Yes",
            )
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
    fn invalid_seed_arguments_and_late_nonmesh_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let before = document.undo_label().map(str::to_owned);
        for input in [
            "ExtractConnectedMeshFaces",
            "ExtractConnectedMeshFaces Face=3",
            "ExtractConnectedMeshFaces Face=0 Angle=-1",
            "ExtractConnectedMeshFaces Face=0 Compare=Other",
            "ExtractConnectedMeshFaces Face=0 Face=1",
            "ExtractConnectedMeshFaces Face=0 FacePoint=0,0,0",
            "ExtractConnectedMeshFaces FacePoint=bad",
        ] {
            assert!(registry.execute(&mut document, input).is_err());
            assert_eq!(document.objects().count(), 1);
            assert_eq!(document.undo_label(), before.as_deref());
        }
        let other = document
            .add_geometry(Geometry::Point(point(20.0, 0.0, 0.0)))
            .unwrap();
        document
            .select_objects_direct([source, other], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "ExtractConnectedMeshFaces Face=0"),
            Err(CommandError::UnsupportedExtractConnectedMeshFacesGeometry)
        ));
        assert_eq!(document.objects().count(), 2);
    }

    #[test]
    fn later_mesh_with_missing_seed_leaves_earlier_mesh_untouched() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let first = selected_mesh(&mut document);
        let second_mesh = TriangleMesh::try_new_faces(
            vec![
                point(3.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(3.0, 1.0, 0.0),
            ],
            vec![MeshFace::Triangle([0, 1, 2])],
            document.tolerance(),
        )
        .unwrap();
        let second = document.add_geometry(Geometry::Mesh(second_mesh)).unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        let before = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "ExtractConnectedMeshFaces Face=1"),
            Err(CommandError::Geometry(
                GeometryError::MeshFaceIndexOutOfRange { .. }
            ))
        ));
        assert_eq!(document.objects().count(), 2);
        assert_eq!(document.undo_label(), before.as_deref());
        let Geometry::Mesh(original) = document.object(first).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(original.face_count(), 3);
        assert_eq!(document.selected_objects().count(), 2);
    }
}
