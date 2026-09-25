use super::*;
use viboceros_geometry::MeshSingleFaceComponents;

const USAGE: &str = "PatchSingleFace (Vertices=a,b,c|Vertex=a Edge=b|Edges=a,b) [JoinMesh=Yes|No]";

pub(super) struct PatchSingleFaceCommand;

impl Command for PatchSingleFaceCommand {
    fn name(&self) -> &'static str {
        "PatchSingleFace"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (components, join_mesh) = parse_options(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedPatchSingleFaceGeometry);
                };
                Ok((object.id(), mesh, object.attributes().clone()))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        if selected.len() > MAX_SPAN_OUTPUT_OBJECTS {
            return Err(too_many_span_outputs("PatchSingleFace"));
        }
        let mut plans = Vec::new();
        for (id, mesh, attributes) in selected {
            let Some(fill) = mesh.patch_single_face(components, document.tolerance())? else {
                continue;
            };
            let (filled, patch) = fill.into_parts();
            plans.push((id, filled, patch, attributes));
        }
        if plans.is_empty() {
            return Err(CommandError::NoPatchableMeshFace);
        }
        let count = plans.len();
        if join_mesh {
            document.replace_object_geometries(
                plans
                    .into_iter()
                    .map(|(id, filled, _, _)| (id, Geometry::Mesh(filled))),
            )?;
            Ok(format!("Patched {count} mesh face(s) in joined mesh(es)"))
        } else {
            let mut output_ids = Vec::new();
            for (_, _, patch, attributes) in plans {
                output_ids.push(
                    document.add_geometry_with_attributes(Geometry::Mesh(patch), attributes)?,
                );
            }
            document.select_objects_direct(output_ids, SelectionMode::Replace)?;
            Ok(format!("Patched {count} mesh face(s) as separate mesh(es)"))
        }
    }
}

fn parse_options(arguments: &[&str]) -> Result<(MeshSingleFaceComponents, bool), CommandError> {
    let mut vertices = None;
    let mut vertex = None;
    let mut edge = None;
    let mut edges = None;
    let mut join_mesh = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments.get(index + 1).ok_or(CommandError::Usage(USAGE))?;
            (argument, *value, 2)
        };
        if option_name_eq(name, "Vertices") && vertices.is_none() {
            vertices = Some(parse_indices::<3>(value)?);
        } else if option_name_eq(name, "Vertex") && vertex.is_none() {
            vertex = Some(parse_index(value)?);
        } else if option_name_eq(name, "Edge") && edge.is_none() {
            edge = Some(parse_index(value)?);
        } else if option_name_eq(name, "Edges") && edges.is_none() {
            edges = Some(parse_indices::<2>(value)?);
        } else if option_name_eq(name, "JoinMesh") && join_mesh.is_none() {
            join_mesh = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        index += consumed;
    }
    let components = match (vertices, vertex, edge, edges) {
        (Some(vertices), None, None, None) => MeshSingleFaceComponents::ThreeVertices(vertices),
        (None, Some(vertex), Some(edge), None) => {
            MeshSingleFaceComponents::VertexAndEdge { vertex, edge }
        }
        (None, None, None, Some(edges)) => MeshSingleFaceComponents::TwoEdges(edges),
        _ => return Err(CommandError::Usage(USAGE)),
    };
    Ok((components, join_mesh.unwrap_or(true)))
}

fn parse_index(value: &str) -> Result<usize, CommandError> {
    value
        .trim_start_matches('_')
        .parse()
        .map_err(|_| CommandError::Usage(USAGE))
}

fn parse_indices<const N: usize>(value: &str) -> Result<[usize; N], CommandError> {
    let values = value
        .split(',')
        .map(parse_index)
        .collect::<Result<Vec<_>, _>>()?;
    let values: [usize; N] = values.try_into().map_err(|_| CommandError::Usage(USAGE))?;
    if values.iter().copied().collect::<BTreeSet<_>>().len() != N {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::MeshFace;

    #[test]
    fn patch_single_face_joins_or_creates_separate_mesh_with_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(1., 1., 0.),
                point(0., 1., 0.),
                point(0., 0., 1.),
            ],
            vec![
                MeshFace::Triangle([0, 1, 4]),
                MeshFace::Triangle([1, 2, 4]),
                MeshFace::Triangle([2, 3, 4]),
                MeshFace::Triangle([3, 0, 4]),
            ],
            document.tolerance(),
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
        document.select_object(id, SelectionMode::Replace).unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "PatchSingleFace Edges=0,5")
                .unwrap(),
            "Patched 1 mesh face(s) in joined mesh(es)"
        );
        let Geometry::Mesh(joined) = document.object(id).unwrap().geometry() else {
            panic!("expected mesh");
        };
        assert_eq!(joined.face_count(), 5);
        assert_eq!(document.undo_label(), Some("PatchSingleFace"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Mesh(mesh.clone())
        );
        assert!(matches!(
            registry.execute(&mut document, "PatchSingleFace Vertices=0,1,4"),
            Err(CommandError::NoPatchableMeshFace)
        ));
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Mesh(mesh.clone())
        );
        assert_eq!(
            registry
                .execute(&mut document, "PatchSingleFace Vertices=0,1,2 JoinMesh=No")
                .unwrap(),
            "Patched 1 mesh face(s) as separate mesh(es)"
        );
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Mesh(mesh)
        );
        assert_eq!(document.objects().count(), 2);
        assert!(
            registry
                .execute(&mut document, "PatchSingleFace Vertices=0,0,2")
                .is_err()
        );
    }
}
