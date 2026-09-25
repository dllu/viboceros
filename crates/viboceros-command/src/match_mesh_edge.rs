use super::*;
use viboceros_geometry::{
    MeshEdgeMatchOptions, MeshEdgePick, MeshJoinOptions, join_meshes, match_mesh_edges,
};

const USAGE: &str = "MatchMeshEdge [DistanceToAdjust=value] [RatchetMode=Yes|No] [AverageVertexesToAdjust=Yes|No] [PickEdges=0,2|0:0,1:2] [Join=Yes|No]";

pub(super) struct MatchMeshEdgeCommand;

impl Command for MatchMeshEdgeCommand {
    fn name(&self) -> &'static str {
        "MatchMeshEdge"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (options, join) = parse_options(arguments, document.tolerance().absolute())?;
        let selected = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedMatchMeshEdgeGeometry);
                };
                Ok((object.id(), mesh))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let ids = selected.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let inputs = selected.iter().map(|(_, mesh)| *mesh).collect::<Vec<_>>();
        let result = match_mesh_edges(&inputs, &options, document.tolerance())?;
        let changed = result
            .meshes
            .iter()
            .zip(&inputs)
            .filter(|(new, old)| new != *old)
            .count();
        if changed == 0 {
            return Ok("Matched 0 mesh edge(s); 0 mesh(es) changed".to_owned());
        }
        let mut joined_count = 0;
        if join {
            let refs = result.meshes.iter().collect::<Vec<_>>();
            let components = join_meshes(
                &refs,
                MeshJoinOptions {
                    join_disjoint: false,
                    alignment_tolerance: 0.,
                    single_precision_matching: false,
                },
            )?;
            let mut replacements = Vec::new();
            let mut deletions = Vec::new();
            for component in components {
                let first = component.source_indices[0];
                if component.source_indices.len() > 1 || component.mesh != *inputs[first] {
                    replacements.push((ids[first], Geometry::Mesh(component.mesh)));
                }
                for &index in &component.source_indices[1..] {
                    deletions.push(ids[index]);
                    joined_count += 1;
                }
            }
            document.replace_object_geometries(replacements)?;
            for id in deletions {
                document.delete_object(id)?;
            }
        } else {
            let replacements = ids
                .into_iter()
                .zip(result.meshes)
                .zip(inputs)
                .filter_map(|((id, mesh), original)| {
                    (mesh != *original).then_some((id, Geometry::Mesh(mesh)))
                })
                .collect::<Vec<_>>();
            document.replace_object_geometries(replacements)?;
        }
        Ok(format!(
            "Matched {} mesh edge(s), aligned {} vertex/vertices; {changed} mesh(es) changed, {joined_count} joined",
            result.split_edges, result.aligned_vertices,
        ))
    }
}

fn parse_options(
    arguments: &[&str],
    default_distance: Real,
) -> Result<(MeshEdgeMatchOptions, bool), CommandError> {
    let mut distance = None;
    let mut average = None;
    let mut ratchet = None;
    let mut join = None;
    let mut picked_edges = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments.get(index + 1).ok_or(CommandError::Usage(USAGE))?;
            (argument, *value, 2)
        };
        if (option_name_eq(name, "DistanceToAdjust") || option_name_eq(name, "Distance"))
            && distance.is_none()
        {
            distance = Some(parse_finite_real(value)?);
        } else if (option_name_eq(name, "AverageVertexesToAdjust")
            || option_name_eq(name, "Average"))
            && average.is_none()
        {
            average = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if option_name_eq(name, "RatchetMode") && ratchet.is_none() {
            ratchet = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if option_name_eq(name, "Join") && join.is_none() {
            join = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if (option_name_eq(name, "PickEdges") || option_name_eq(name, "Edges"))
            && picked_edges.is_none()
        {
            let parts = value.trim_start_matches('_').split(',').collect::<Vec<_>>();
            if parts.is_empty() {
                return Err(CommandError::Usage(USAGE));
            }
            if parts[0].contains(':') {
                let edges = parts
                    .into_iter()
                    .map(|part| {
                        let (mesh, edge) =
                            part.split_once(':').ok_or(CommandError::Usage(USAGE))?;
                        Ok((
                            mesh.parse::<usize>()
                                .map_err(|_| CommandError::Usage(USAGE))?,
                            edge.parse::<usize>()
                                .map_err(|_| CommandError::Usage(USAGE))?,
                        ))
                    })
                    .collect::<Result<Vec<_>, CommandError>>()?;
                if edges.iter().copied().collect::<BTreeSet<_>>().len() != edges.len() {
                    return Err(CommandError::Usage(USAGE));
                }
                picked_edges = Some(MeshEdgePick::PerMesh(edges));
            } else {
                let edges = parts
                    .into_iter()
                    .map(|part| {
                        part.parse::<usize>()
                            .map_err(|_| CommandError::Usage(USAGE))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if edges.iter().copied().collect::<BTreeSet<_>>().len() != edges.len() {
                    return Err(CommandError::Usage(USAGE));
                }
                picked_edges = Some(MeshEdgePick::Shared(edges));
            }
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        index += consumed;
    }
    let distance = distance.unwrap_or(default_distance);
    if distance <= 0. {
        return Err(CommandError::Usage(USAGE));
    }
    Ok((
        MeshEdgeMatchOptions {
            distance,
            average: average.unwrap_or(false),
            ratchet: ratchet.unwrap_or(false),
            edge_selection: picked_edges.unwrap_or(MeshEdgePick::All),
        },
        join.unwrap_or(false),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_t_junction_and_undoes_as_one_edit() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let edge_mesh = TriangleMesh::try_new(
            vec![point(0., 0.), point(2., 0.), point(0., -1.)],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap();
        let tip_mesh = TriangleMesh::try_new(
            vec![point(1., 0.04), point(1., 1.), point(2., 1.)],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap();
        let a = document
            .add_geometry(Geometry::Mesh(edge_mesh.clone()))
            .unwrap();
        let b = document
            .add_geometry(Geometry::Mesh(tip_mesh.clone()))
            .unwrap();
        document
            .select_objects_direct([a, b], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "MatchMeshEdge DistanceToAdjust=0.05")
                .unwrap(),
            "Matched 1 mesh edge(s), aligned 0 vertex/vertices; 2 mesh(es) changed, 0 joined"
        );
        assert_eq!(document.undo_label(), Some("MatchMeshEdge"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(a).unwrap().geometry(),
            &Geometry::Mesh(edge_mesh.clone())
        );
        assert_eq!(
            document.object(b).unwrap().geometry(),
            &Geometry::Mesh(tip_mesh.clone())
        );
        assert!(
            registry
                .execute(
                    &mut document,
                    "MatchMeshEdge DistanceToAdjust=0.05 PickEdges=0:0,1:1"
                )
                .unwrap()
                .contains("Matched 1 mesh edge(s)")
        );
    }

    #[test]
    fn joins_matched_neighbor_meshes_and_restores_both_on_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let left = TriangleMesh::try_new(
            vec![point(0., 0.), point(1., 0.), point(1., 1.)],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap();
        let right = TriangleMesh::try_new(
            vec![point(1.04, 0.), point(2., 0.), point(1.04, 1.)],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap();
        let a = document.add_geometry(Geometry::Mesh(left.clone())).unwrap();
        let b = document
            .add_geometry(Geometry::Mesh(right.clone()))
            .unwrap();
        document
            .select_objects_direct([a, b], SelectionMode::Replace)
            .unwrap();
        let message = registry
            .execute(
                &mut document,
                "MatchMeshEdge DistanceToAdjust=0.05 Join=Yes",
            )
            .unwrap();
        assert!(message.contains("1 joined"), "{message}");
        assert!(document.object(a).is_some());
        assert!(document.object(b).is_none());
        assert_eq!(document.objects().count(), 1);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(a).unwrap().geometry(),
            &Geometry::Mesh(left)
        );
        assert_eq!(
            document.object(b).unwrap().geometry(),
            &Geometry::Mesh(right)
        );
    }

    #[test]
    fn rejects_mixed_selection_and_invalid_options_without_editing() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let mesh = TriangleMesh::try_new(
            vec![point(0., 0.), point(1., 0.), point(0., 1.)],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap();
        let mesh_id = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
        let point_id = document
            .add_geometry(Geometry::Point(point(4., 0.)))
            .unwrap();
        document
            .select_objects_direct([mesh_id, point_id], SelectionMode::Replace)
            .unwrap();
        let history = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "MatchMeshEdge DistanceToAdjust=0.1"),
            Err(CommandError::UnsupportedMatchMeshEdgeGeometry)
        ));
        document
            .select_object(mesh_id, SelectionMode::Replace)
            .unwrap();
        for command in [
            "MatchMeshEdge DistanceToAdjust=0",
            "MatchMeshEdge RatchetMode=Maybe",
            "MatchMeshEdge PickEdges=0,0",
            "MatchMeshEdge PickEdges=0:0,0:0",
            "MatchMeshEdge PickEdges=0:0,1",
            "MatchMeshEdge PickEdges=2:0",
            "MatchMeshEdge PickEdges=4",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(
            document.object(mesh_id).unwrap().geometry(),
            &Geometry::Mesh(mesh)
        );
        assert_eq!(document.undo_label(), history.as_deref());
    }
}
