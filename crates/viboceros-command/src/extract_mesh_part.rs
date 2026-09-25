//! Extract mesh regions bounded by naked, unwelded, or nonmanifold edges.

use super::*;
use viboceros_geometry::MeshPartBoundary;

const USAGE: &str = "ExtractMeshPart (Face=index|Faces=0,2,...|Faces=All) [ExtractWholeDisjointParts=Yes|No] [ExtractToNonManifoldEdges=Yes|No] [JoinOutput=Yes|No] [MakeCopy=Yes|No] [BorderOnly=Yes|No]";

struct Options {
    seeds: SurfaceFaceIndices,
    boundary: MeshPartBoundary,
    join_output: bool,
    make_copy: bool,
    border_only: bool,
}

struct Plan {
    source: ObjectId,
    remainder: Option<TriangleMesh>,
    outputs: Vec<Geometry>,
    attributes: ObjectAttributes,
    groups: Vec<GroupId>,
}

pub(super) struct ExtractMeshPartCommand;

impl Command for ExtractMeshPartCommand {
    fn name(&self) -> &'static str {
        "ExtractMeshPart"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        let tolerance = document.tolerance();
        let mut source_count = 0_usize;
        let mut face_count = 0_usize;
        let mut output_count = 0_usize;
        let mut plans = Vec::new();
        for object in document.selected_objects() {
            source_count += 1;
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(CommandError::UnsupportedExtractMeshPartGeometry);
            };
            let seeds = match &options.seeds {
                SurfaceFaceIndices::All => (0..mesh.face_count()).collect::<Vec<_>>(),
                SurfaceFaceIndices::Indices(indices) => indices.clone(),
            };
            let groups = mesh.mesh_part_face_groups(&seeds, options.boundary)?;
            let mut selected = groups.into_iter().flatten().collect::<Vec<_>>();
            selected.sort_unstable();
            face_count = face_count
                .checked_add(selected.len())
                .ok_or_else(|| too_many_span_outputs("ExtractMeshPart"))?;
            let (remainder, extracted) = mesh.extract_faces(&selected)?.into_parts();
            let outputs = if options.border_only {
                let mut lines = Vec::new();
                for border in extracted.boundary_polylines(tolerance)? {
                    for pair in border.vertices().windows(2) {
                        if output_count + lines.len() == MAX_SPAN_OUTPUT_OBJECTS {
                            return Err(too_many_span_outputs("ExtractMeshPart"));
                        }
                        lines.push(Geometry::Line(LineSegment::try_new(
                            pair[0], pair[1], tolerance,
                        )?));
                    }
                }
                lines
            } else if options.join_output {
                vec![Geometry::Mesh(extracted)]
            } else {
                if extracted.face_count() > MAX_SPAN_OUTPUT_OBJECTS - output_count {
                    return Err(too_many_span_outputs("ExtractMeshPart"));
                }
                extracted
                    .individual_face_meshes()
                    .into_iter()
                    .map(Geometry::Mesh)
                    .collect()
            };
            output_count = output_count
                .checked_add(outputs.len())
                .filter(|&count| count <= MAX_SPAN_OUTPUT_OBJECTS)
                .ok_or_else(|| too_many_span_outputs("ExtractMeshPart"))?;
            plans.push(Plan {
                source: object.id(),
                remainder,
                outputs,
                attributes: object.attributes().clone(),
                groups: object.group_ids().to_vec(),
            });
        }
        if source_count == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        if output_count == 0 {
            return Err(CommandError::NoMeshPartBorders);
        }
        if !options.border_only && !options.make_copy {
            document.clear_selection();
            document.replace_object_geometries(plans.iter().map(|plan| {
                let replacement = plan.remainder.as_ref().map_or_else(
                    || plan.outputs[0].clone(),
                    |remainder| Geometry::Mesh(remainder.clone()),
                );
                (plan.source, replacement)
            }))?;
        }
        let mut results = Vec::with_capacity(output_count);
        for plan in plans {
            let reuse_source =
                !options.border_only && !options.make_copy && plan.remainder.is_none();
            for (index, output) in plan.outputs.into_iter().enumerate() {
                if reuse_source && index == 0 {
                    results.push(plan.source);
                } else {
                    let id =
                        document.add_geometry_with_attributes(output, plan.attributes.clone())?;
                    document.set_object_group_memberships(id, plan.groups.clone())?;
                    results.push(id);
                }
            }
        }
        document.select_command_results(results)?;
        Ok(format!(
            "Extracted {face_count} mesh face(s) from {source_count} mesh(es){}",
            if options.border_only {
                " as border lines"
            } else if options.make_copy {
                "; source faces copied"
            } else {
                "; source faces removed"
            }
        ))
    }
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let mut seeds = None;
    let mut whole_disjoint = None;
    let mut to_nonmanifold = None;
    let mut join_output = None;
    let mut make_copy = None;
    let mut border_only = None;
    for &argument in arguments {
        let Some((key, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(USAGE));
        };
        if key.eq_ignore_ascii_case("Face") && seeds.is_none() {
            seeds = Some(SurfaceFaceIndices::Indices(vec![
                value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(USAGE))?,
            ]));
        } else if key.eq_ignore_ascii_case("Faces") && seeds.is_none() {
            seeds = Some(parse_surface_face_indices(value, USAGE)?);
        } else if key.eq_ignore_ascii_case("ExtractWholeDisjointParts") && whole_disjoint.is_none()
        {
            whole_disjoint = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("ExtractToNonManifoldEdges") && to_nonmanifold.is_none()
        {
            to_nonmanifold = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("JoinOutput") && join_output.is_none() {
            join_output = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
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
        seeds: seeds.ok_or(CommandError::Usage(USAGE))?,
        boundary,
        join_output: join_output.unwrap_or(true),
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
        assert_eq!(selected.len(), 4);
        assert!(
            selected
                .iter()
                .all(|object| object.group_ids().contains(&group))
        );
        assert!(
            selected
                .iter()
                .all(|object| matches!(object.geometry(), Geometry::Line(_)))
        );
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
            "ExtractMeshPart Face=0 Faces=1",
            "ExtractMeshPart Faces=0,0",
            "ExtractMeshPart Faces=0,3",
            "ExtractMeshPart Face=0 JoinOutput=Maybe",
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

    #[test]
    fn multiple_seeds_deduplicate_regions_and_join_or_split_outputs() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let group = document.add_group(None, [source]).unwrap();
        registry
            .execute(
                &mut document,
                "ExtractMeshPart Faces=1,0 MakeCopy=Yes JoinOutput=Yes",
            )
            .unwrap();
        let joined = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(joined.len(), 1);
        let Geometry::Mesh(joined_mesh) = joined[0].geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(joined_mesh.face_count(), 2);
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "ExtractMeshPart Face=0 JoinOutput=No")
            .unwrap();
        let split = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(split.len(), 2);
        assert!(
            split
                .iter()
                .all(|object| object.group_ids().contains(&group))
        );
        assert!(split.iter().all(|object| matches!(
            object.geometry(),
            Geometry::Mesh(mesh) if mesh.face_count() == 1
        )));
        let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(remainder.face_count(), 1);
        document.undo().unwrap();
        let Geometry::Mesh(restored) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(restored.face_count(), 3);
    }

    #[test]
    fn all_faces_with_separate_output_reuses_source_for_one_face() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        registry
            .execute(&mut document, "ExtractMeshPart Faces=All JoinOutput=No")
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 3);
        assert!(selected.iter().any(|object| object.id() == source));
        assert!(selected.iter().all(|object| matches!(
            object.geometry(),
            Geometry::Mesh(mesh) if mesh.face_count() == 1
        )));
        document.undo().unwrap();
        let Geometry::Mesh(restored) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(restored.face_count(), 3);
    }
}
