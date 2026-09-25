//! Offset selected polygon meshes by vertex normals or a world vector.

use super::*;
use viboceros_geometry::MeshOffsetDirection;

const USAGE: &str = "OffsetMesh distance [DirectionMethod=UseVertexNormals|UserSelectedDirection] [Direction=x,y,z] [AverageNormals=Yes|No] [CPlaneNormal=x,y,z] [FlipAll=Yes|No] [Solid=Yes|No] [BothSides=Yes|No] [AllowDisjoint=Yes|No] [DeleteInput=Yes|No]";

pub(super) struct OffsetMeshCommand;

#[derive(Clone, Copy)]
struct Options {
    distance: Real,
    direction: MeshOffsetDirection,
    solid: bool,
    both_sides: bool,
    allow_disjoint: bool,
    delete_input: bool,
}

impl Command for OffsetMeshCommand {
    fn name(&self) -> &'static str {
        "OffsetMesh"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let options = parse(
            arguments,
            document.tolerance(),
            context.construction_plane.z_axis(),
        )?;
        let sources = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry().clone()))
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut outputs = Vec::new();
        for (id, geometry) in &sources {
            let Geometry::Mesh(mesh) = geometry else {
                return Err(CommandError::UnsupportedOffsetMeshGeometry);
            };
            let result = mesh.offset_mesh(
                options.distance,
                options.direction,
                options.both_sides,
                options.solid,
                document.tolerance(),
            )?;
            if options.allow_disjoint {
                if outputs.len() == MAX_SPAN_OUTPUT_OBJECTS {
                    return Err(too_many_span_outputs("OffsetMesh"));
                }
                outputs.push((*id, Geometry::Mesh(result)));
            } else {
                outputs.extend(
                    result
                        .try_disjoint_pieces(MAX_SPAN_OUTPUT_OBJECTS - outputs.len())?
                        .into_iter()
                        .map(|piece| (*id, Geometry::Mesh(piece))),
                );
            }
        }
        let count = outputs.len();
        let ids = document.copy_object_pieces_into_source_groups(outputs)?;
        if options.delete_input {
            document.delete_objects(sources.into_iter().map(|(id, _)| id))?;
        }
        document.select_objects_direct(ids, SelectionMode::Replace)?;
        Ok(format!("Offset {count} mesh(es)"))
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(
            arguments,
            Tolerance::NUMERICAL_VALIDATION,
            CommandContext::default().construction_plane.z_axis(),
        )?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Mesh,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}

fn parse(
    arguments: &[&str],
    tolerance: Tolerance,
    cplane_fallback: UnitVector3,
) -> Result<Options, CommandError> {
    let (first, rest) = arguments.split_first().ok_or(CommandError::Usage(USAGE))?;
    let distance = if let Some((name, value)) = first.split_once('=') {
        if !option_name_eq(name, "Distance") {
            return Err(CommandError::Usage(USAGE));
        }
        parse_finite_real(value)?
    } else {
        parse_finite_real(first)?
    };
    if distance.abs() <= tolerance.absolute() {
        return Err(CommandError::Usage(USAGE));
    }
    let (mut method, mut vector, mut average, mut cplane_normal) = (None, None, None, None);
    let (mut flip_all, mut solid, mut both_sides, mut allow_disjoint, mut delete_input) =
        (None, None, None, None, None);
    for argument in rest {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        if option_name_eq(name, "DirectionMethod") {
            let mode = if option_name_eq(value, "UseVertexNormals") {
                false
            } else if option_name_eq(value, "UserSelectedDirection") {
                true
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if method.replace(mode).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(name, "Direction") || option_name_eq(name, "CPlaneNormal") {
            let parts = value.split(',').collect::<Vec<_>>();
            let [x, y, z] = parts.as_slice() else {
                return Err(CommandError::Usage(USAGE));
            };
            let coordinate =
                |text: &str| text.parse::<Real>().map_err(|_| CommandError::Usage(USAGE));
            let normal = Vector3::try_new(coordinate(x)?, coordinate(y)?, coordinate(z)?)
                .and_then(Vector3::normalized_nonzero)
                .map_err(|_| CommandError::Usage(USAGE))?;
            let slot = if option_name_eq(name, "Direction") {
                &mut vector
            } else {
                &mut cplane_normal
            };
            if slot.replace(normal).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else {
            let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            let slot = if option_name_eq(name, "AverageNormals") {
                &mut average
            } else if option_name_eq(name, "FlipAll") {
                &mut flip_all
            } else if option_name_eq(name, "Solid") {
                &mut solid
            } else if option_name_eq(name, "BothSides") {
                &mut both_sides
            } else if option_name_eq(name, "AllowDisjoint") {
                &mut allow_disjoint
            } else if option_name_eq(name, "DeleteInput") {
                &mut delete_input
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if slot.replace(value).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        }
    }
    if method == Some(false) && vector.is_some()
        || method == Some(true) && average == Some(true)
        || method == Some(true) && vector.is_none()
        || method != Some(true) && vector.is_some()
        || cplane_normal.is_some() && average != Some(true)
    {
        return Err(CommandError::Usage(USAGE));
    }
    let fallback = cplane_normal.unwrap_or(cplane_fallback);
    let direction = if let Some(vector) = vector {
        MeshOffsetDirection::Vector(vector)
    } else if average.unwrap_or(false) {
        MeshOffsetDirection::AverageNormals { fallback }
    } else {
        MeshOffsetDirection::VertexNormals
    };
    Ok(Options {
        distance: if flip_all.unwrap_or(false) {
            -distance
        } else {
            distance
        },
        direction,
        solid: solid.unwrap_or(false),
        both_sides: both_sides.unwrap_or(false),
        allow_disjoint: allow_disjoint.unwrap_or(false),
        delete_input: delete_input.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::MeshFace;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn selected_triangle(document: &mut Document) -> ObjectId {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![MeshFace::Triangle([0, 1, 2])],
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
    fn solid_offset_copies_group_and_undoes_deletion() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_triangle(&mut document);
        let group = document.add_group(None, [source]).unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "OffsetMesh 2 Solid=Yes DeleteInput=Yes")
                .unwrap(),
            "Offset 1 mesh(es)"
        );
        assert!(document.object(source).is_none());
        let result = document.selected_objects().next().unwrap();
        assert!(result.group_ids().contains(&group));
        let Geometry::Mesh(shell) = result.geometry() else {
            panic!("mesh expected")
        };
        assert!(shell.topology().is_solid());
        assert!((shell.signed_volume().unwrap() - 1.0).abs() < 1e-12);
        document.undo().unwrap();
        assert!(document.object(source).is_some());
        assert_eq!(document.objects().count(), 1);
    }

    #[test]
    fn both_sides_and_disjoint_option_control_output_count() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_triangle(&mut document);
        registry
            .execute(&mut document, "OffsetMesh 1 BothSides=Yes")
            .unwrap();
        assert_eq!(document.selected_objects().count(), 2);
        document.undo().unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "OffsetMesh 1 BothSides=Yes AllowDisjoint=Yes",
            )
            .unwrap();
        let result = document.selected_objects().next().unwrap();
        let Geometry::Mesh(mesh) = result.geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(mesh.disjoint_pieces().len(), 2);
        assert_eq!(document.selected_objects().count(), 1);
    }

    #[test]
    fn invalid_options_and_nonmesh_selection_do_not_edit_document() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        selected_triangle(&mut document);
        for input in [
            "OffsetMesh 0",
            "OffsetMesh 1 DirectionMethod=UserSelectedDirection",
            "OffsetMesh 1 Direction=0,0,1",
            "OffsetMesh 1 Solid=Yes Solid=No",
            "OffsetMesh 1 DirectionMethod=UserSelectedDirection Direction=0,0,0",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            assert_eq!(document.objects().count(), 1);
        }
        let point_id = document
            .add_geometry(Geometry::Point(point(4.0, 0.0, 0.0)))
            .unwrap();
        document
            .select_objects_direct([point_id], SelectionMode::Add)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "OffsetMesh 1"),
            Err(CommandError::UnsupportedOffsetMeshGeometry)
        ));
        assert_eq!(document.objects().count(), 2);
    }

    #[test]
    fn average_normal_fallback_uses_command_construction_plane() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let triangle = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap();
        let opposed = TriangleMesh::try_append(&[&triangle, &triangle.reversed()]).unwrap();
        let id = document.add_geometry(Geometry::Mesh(opposed)).unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let plane = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            document.tolerance(),
        )
        .unwrap();
        registry
            .execute_in_context(
                &mut document,
                "OffsetMesh 2 AverageNormals=Yes AllowDisjoint=Yes",
                CommandContext {
                    construction_plane: plane,
                },
            )
            .unwrap();
        let Geometry::Mesh(result) = document.selected_objects().next().unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert!(result.vertices().iter().all(|point| point.x() >= 2.0));
        assert!(result.vertices().iter().all(|point| point.z() == 0.0));
    }

    #[test]
    fn typed_direction_and_flip_all_move_mesh_to_opposite_side() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_triangle(&mut document);
        registry
            .execute(
                &mut document,
                "OffsetMesh 1 DirectionMethod=UserSelectedDirection Direction=0,0,1 FlipAll=Yes",
            )
            .unwrap();
        let Geometry::Mesh(result) = document.selected_objects().next().unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert!(result.vertices().iter().all(|point| point.z() == -1.0));
        assert!(document.object(source).is_some());
    }
}
