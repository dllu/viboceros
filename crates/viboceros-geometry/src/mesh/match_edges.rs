//! Match open mesh edges by aligning vertices and splitting near T junctions.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum MeshEdgePick {
    All,
    Shared(Vec<usize>),
    PerMesh(Vec<(usize, usize)>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshEdgeMatchOptions {
    pub distance: Real,
    pub average: bool,
    pub ratchet: bool,
    /// Initial topology-edge indices to consider on the selected meshes.
    pub edge_selection: MeshEdgePick,
}

pub struct MeshEdgeMatchResult {
    pub meshes: Vec<TriangleMesh>,
    pub aligned_vertices: usize,
    pub split_edges: usize,
}

#[derive(Clone, Copy)]
struct MatchCandidate {
    source_mesh: usize,
    source_raw: usize,
    source_topology: usize,
    source_point: Point3,
    target_mesh: usize,
    target_edge: usize,
    parameter: Real,
    projected: Point3,
    separation: Real,
}

#[derive(Clone, Copy)]
struct SearchEdge {
    mesh: usize,
    index: usize,
    vertices: [usize; 2],
    face: usize,
    points: [Point3; 2],
    minimum: [Real; 3],
    maximum: [Real; 3],
}

struct EdgeNode {
    minimum: [Real; 3],
    maximum: [Real; 3],
    left: Option<usize>,
    right: Option<usize>,
    edge: Option<usize>,
}

/// Matches selected open mesh edges. Each original source vertex causes at
/// most one edge split, so generated split vertices cannot trigger an
/// unbounded cascade. All output meshes are validated before returning.
pub fn match_mesh_edges(
    meshes: &[&TriangleMesh],
    options: &MeshEdgeMatchOptions,
    tolerance: Tolerance,
) -> Result<MeshEdgeMatchResult, GeometryError> {
    if !options.distance.is_finite() || options.distance <= 0. {
        return Err(GeometryError::InvalidTolerance);
    }
    let mut current = meshes
        .iter()
        .map(|mesh| (*mesh).clone())
        .collect::<Vec<_>>();
    let mut masks = Vec::new();
    if let MeshEdgePick::PerMesh(picks) = &options.edge_selection
        && let Some(&(mesh, _)) = picks.iter().find(|(mesh, _)| *mesh >= current.len())
    {
        return Err(GeometryError::MeshSelectionIndexOutOfRange {
            mesh,
            mesh_count: current.len(),
        });
    }
    for (mesh_index, mesh) in current.iter().enumerate() {
        let data = mesh.topology_data();
        let mut mask =
            vec![matches!(options.edge_selection, MeshEdgePick::All); mesh.vertices.len()];
        let picks: Vec<usize> = match &options.edge_selection {
            MeshEdgePick::All => Vec::new(),
            MeshEdgePick::Shared(edges) => edges.clone(),
            MeshEdgePick::PerMesh(picks) => picks
                .iter()
                .filter_map(|&(mesh, edge)| (mesh == mesh_index).then_some(edge))
                .collect(),
        };
        for edge in picks {
            let Some((_, incidence)) = data.edges.iter().nth(edge) else {
                return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                    edge,
                    edge_count: data.edges.len(),
                });
            };
            if incidence.count == 1 {
                for use_ in incidence.uses() {
                    for raw in use_.raw_vertices {
                        mask[raw as usize] = true;
                    }
                }
            }
        }
        masks.push(mask);
    }
    let mut aligned_vertices = 0;
    let mut split_edges = 0;
    let mut processed = BTreeSet::new();
    let passes: &[Real] = if options.ratchet {
        &[0.25, 0.5, 0.75, 1.]
    } else {
        &[1.]
    };
    for &fraction in passes {
        let threshold = options.distance * fraction;
        if threshold <= 0. {
            continue;
        }
        let references = current.iter().collect::<Vec<_>>();
        let aligned = alignment::align_mesh_vertices_with_masks(
            &references,
            threshold,
            options.average,
            &masks,
            tolerance,
        )?;
        aligned_vertices += aligned.iter().map(|(_, count)| count).sum::<usize>();
        current = aligned.into_iter().map(|(mesh, _)| mesh).collect();
        let mut rejected = BTreeSet::new();
        while let Some(candidate) =
            closest_match_candidate(&current, &masks, &processed, &rejected, threshold)?
        {
            let key = (
                candidate.source_mesh,
                candidate.source_raw,
                candidate.target_mesh,
                candidate.target_edge,
            );
            match apply_match(
                &mut current,
                &mut masks,
                candidate,
                options.average,
                tolerance,
            ) {
                Ok(true) => {
                    split_edges += 1;
                    let final_point = if options.average {
                        candidate.source_point.midpoint(candidate.projected)?
                    } else {
                        candidate.projected
                    };
                    let key = point_key(final_point);
                    processed.insert((candidate.source_mesh, key));
                    processed.insert((candidate.target_mesh, key));
                    rejected.clear();
                }
                Ok(false)
                | Err(GeometryError::DegenerateTriangle { .. })
                | Err(GeometryError::DegenerateQuad { .. })
                | Err(GeometryError::Degenerate { .. }) => {
                    rejected.insert(key);
                }
                Err(error) => return Err(error),
            }
        }
    }
    Ok(MeshEdgeMatchResult {
        meshes: current,
        aligned_vertices,
        split_edges,
    })
}

fn closest_match_candidate(
    meshes: &[TriangleMesh],
    masks: &[Vec<bool>],
    processed: &BTreeSet<(usize, [u64; 3])>,
    rejected: &BTreeSet<(usize, usize, usize, usize)>,
    distance: Real,
) -> Result<Option<MatchCandidate>, GeometryError> {
    let data = meshes
        .iter()
        .map(TriangleMesh::topology_data)
        .collect::<Vec<_>>();
    let mut edges = Vec::new();
    for (mesh, target) in data.iter().enumerate() {
        for (index, (&(a, b), incidence)) in target.edges.iter().enumerate() {
            if incidence.count != 1
                || !incidence
                    .first_use
                    .unwrap()
                    .raw_vertices
                    .iter()
                    .all(|&raw| masks[mesh][raw as usize])
            {
                continue;
            }
            let points = [target.topological_points[a], target.topological_points[b]];
            let first = points[0].to_array();
            let second = points[1].to_array();
            edges.push(SearchEdge {
                mesh,
                index,
                vertices: [a, b],
                face: incidence.first_use.unwrap().face,
                points,
                minimum: std::array::from_fn(|axis| first[axis].min(second[axis])),
                maximum: std::array::from_fn(|axis| first[axis].max(second[axis])),
            });
        }
    }
    let mut indices = (0..edges.len()).collect::<Vec<_>>();
    let mut nodes = Vec::with_capacity(edges.len().saturating_mul(2));
    let root = build_edge_tree(&mut indices, &edges, &mut nodes);
    let mut best: Option<MatchCandidate> = None;
    for (source_mesh, source) in data.iter().enumerate() {
        let mut raw_by_topology = vec![None; source.topological_vertex_count];
        for (raw, &topology) in source.topological_vertices.iter().enumerate() {
            if masks[source_mesh][raw] {
                raw_by_topology[topology].get_or_insert(raw);
            }
        }
        let mut naked = vec![false; source.topological_vertex_count];
        for (&(a, b), incidence) in &source.edges {
            if incidence.count == 1 {
                naked[a] = true;
                naked[b] = true;
            }
        }
        for (source_topology, raw) in raw_by_topology.into_iter().enumerate() {
            let Some(source_raw) = raw else {
                continue;
            };
            if !naked[source_topology]
                || processed.contains(&(
                    source_mesh,
                    point_key(source.topological_points[source_topology]),
                ))
            {
                continue;
            }
            let point = source.topological_points[source_topology];
            if let Some(root) = root {
                search_edge_tree(
                    root,
                    &nodes,
                    &edges,
                    &data,
                    meshes,
                    rejected,
                    distance,
                    source_mesh,
                    source_raw,
                    source_topology,
                    point,
                    &mut best,
                )?;
            }
        }
    }
    Ok(best)
}

fn build_edge_tree(
    indices: &mut [usize],
    edges: &[SearchEdge],
    nodes: &mut Vec<EdgeNode>,
) -> Option<usize> {
    if indices.is_empty() {
        return None;
    }
    let mut minimum = [Real::INFINITY; 3];
    let mut maximum = [Real::NEG_INFINITY; 3];
    for &index in indices.iter() {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(edges[index].minimum[axis]);
            maximum[axis] = maximum[axis].max(edges[index].maximum[axis]);
        }
    }
    let node_index = nodes.len();
    nodes.push(EdgeNode {
        minimum,
        maximum,
        left: None,
        right: None,
        edge: None,
    });
    if indices.len() == 1 {
        nodes[node_index].edge = Some(indices[0]);
        return Some(node_index);
    }
    let axis = (0..3)
        .max_by(|&a, &b| (maximum[a] - minimum[a]).total_cmp(&(maximum[b] - minimum[b])))
        .unwrap();
    let middle = indices.len() / 2;
    indices.select_nth_unstable_by(middle, |&a, &b| {
        edges[a].minimum[axis]
            .midpoint(edges[a].maximum[axis])
            .total_cmp(&edges[b].minimum[axis].midpoint(edges[b].maximum[axis]))
            .then_with(|| a.cmp(&b))
    });
    let (left, right) = indices.split_at_mut(middle);
    nodes[node_index].left = build_edge_tree(left, edges, nodes);
    nodes[node_index].right = build_edge_tree(right, edges, nodes);
    Some(node_index)
}

fn bbox_distance(point: Point3, node: &EdgeNode) -> Real {
    let point = point.to_array();
    let offset = std::array::from_fn::<_, 3, _>(|axis| {
        if point[axis] < node.minimum[axis] {
            node.minimum[axis] - point[axis]
        } else if point[axis] > node.maximum[axis] {
            point[axis] - node.maximum[axis]
        } else {
            0.
        }
    });
    offset[0].hypot(offset[1]).hypot(offset[2])
}

#[allow(clippy::too_many_arguments)]
fn search_edge_tree(
    node_index: usize,
    nodes: &[EdgeNode],
    edges: &[SearchEdge],
    data: &[MeshTopologyData],
    meshes: &[TriangleMesh],
    rejected: &BTreeSet<(usize, usize, usize, usize)>,
    distance: Real,
    source_mesh: usize,
    source_raw: usize,
    source_topology: usize,
    point: Point3,
    best: &mut Option<MatchCandidate>,
) -> Result<(), GeometryError> {
    let node = &nodes[node_index];
    if bbox_distance(point, node) > best.map_or(distance, |best| best.separation) {
        return Ok(());
    }
    if let Some(edge_index) = node.edge {
        let edge = edges[edge_index];
        if rejected.contains(&(source_mesh, source_raw, edge.mesh, edge.index))
            || source_mesh == edge.mesh && edge.vertices.contains(&source_topology)
        {
            return Ok(());
        }
        if source_mesh == edge.mesh {
            let face = &meshes[edge.mesh].faces[edge.face];
            if face
                .indices()
                .iter()
                .any(|&raw| data[edge.mesh].topological_vertices[raw as usize] == source_topology)
            {
                return Ok(());
            }
        }
        let line =
            LineSegment::try_new(edge.points[0], edge.points[1], Tolerance::MESH_VALIDATION)?;
        let parameter = line.closest_parameter(point, Tolerance::MESH_VALIDATION)?;
        if parameter <= 0. || parameter >= 1. {
            return Ok(());
        }
        let projected = line.point_at(parameter)?;
        let Ok(separation) = point.distance_to(projected) else {
            return Ok(());
        };
        if separation >= distance {
            return Ok(());
        }
        let candidate = MatchCandidate {
            source_mesh,
            source_raw,
            source_topology,
            source_point: point,
            target_mesh: edge.mesh,
            target_edge: edge.index,
            parameter,
            projected,
            separation,
        };
        if best.is_none_or(|prior| {
            candidate.separation < prior.separation
                || candidate.separation == prior.separation
                    && (source_mesh, source_raw, edge.mesh, edge.index)
                        < (
                            prior.source_mesh,
                            prior.source_raw,
                            prior.target_mesh,
                            prior.target_edge,
                        )
        }) {
            *best = Some(candidate);
        }
        return Ok(());
    }
    let children = match (node.left, node.right) {
        (Some(a), Some(b))
            if bbox_distance(point, &nodes[a]) <= bbox_distance(point, &nodes[b]) =>
        {
            [Some(a), Some(b)]
        }
        (Some(a), Some(b)) => [Some(b), Some(a)],
        (Some(a), None) | (None, Some(a)) => [Some(a), None],
        (None, None) => [None, None],
    };
    for child in children.into_iter().flatten() {
        search_edge_tree(
            child,
            nodes,
            edges,
            data,
            meshes,
            rejected,
            distance,
            source_mesh,
            source_raw,
            source_topology,
            point,
            best,
        )?;
    }
    Ok(())
}

fn apply_match(
    meshes: &mut [TriangleMesh],
    masks: &mut [Vec<bool>],
    candidate: MatchCandidate,
    average: bool,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let source = &meshes[candidate.source_mesh];
    let source_data = source.topology_data();
    let source_point = source_data.topological_points[candidate.source_topology];
    let target_point = if average {
        source_point.midpoint(candidate.projected)?
    } else {
        candidate.projected
    };
    let selected_points = meshes[candidate.target_mesh]
        .vertices
        .iter()
        .zip(&masks[candidate.target_mesh])
        .filter_map(|(point, &selected)| selected.then_some(point_key(*point)))
        .collect::<BTreeSet<_>>();
    let Some(split) = meshes[candidate.target_mesh].split_topology_edge(
        candidate.target_edge,
        candidate.parameter,
        tolerance,
    )?
    else {
        return Ok(false);
    };
    let mut updates = BTreeMap::<usize, Vec<(usize, Point3)>>::new();
    if target_point != source_point {
        let source_raw = if candidate.source_mesh == candidate.target_mesh {
            split
                .vertices
                .iter()
                .enumerate()
                .filter_map(|(raw, &point)| (point == source_point).then_some(raw))
                .collect::<Vec<_>>()
        } else {
            source_data
                .topological_vertices
                .iter()
                .enumerate()
                .filter_map(|(raw, &topology)| {
                    (topology == candidate.source_topology).then_some(raw)
                })
                .collect::<Vec<_>>()
        };
        if source_raw.is_empty() {
            return Ok(false);
        }
        updates
            .entry(candidate.source_mesh)
            .or_default()
            .extend(source_raw.into_iter().map(|raw| (raw, target_point)));
    }
    if average && target_point != candidate.projected {
        let generated = (0..split.vertices.len())
            .filter(|&raw| split.vertices[raw] == candidate.projected)
            .collect::<Vec<_>>();
        if generated.is_empty() {
            return Ok(false);
        }
        updates
            .entry(candidate.target_mesh)
            .or_default()
            .extend(generated.into_iter().map(|raw| (raw, target_point)));
    }
    let mut staged = BTreeMap::new();
    staged.insert(candidate.target_mesh, split);
    for (mesh_index, raw_updates) in updates {
        let mesh = staged.get(&mesh_index).unwrap_or(&meshes[mesh_index]);
        let mut vertices = mesh.vertices.clone();
        for (raw, point) in raw_updates {
            vertices[raw] = point;
        }
        let rebuilt = TriangleMesh::try_new_faces(vertices, mesh.faces.clone(), tolerance)?
            .try_with_vertex_colors(mesh.vertex_colors.clone())?
            .try_with_ngons(mesh.ngons.clone())?;
        staged.insert(mesh_index, rebuilt);
    }
    masks[candidate.target_mesh] = staged[&candidate.target_mesh]
        .vertices
        .iter()
        .map(|&point| selected_points.contains(&point_key(point)) || point == target_point)
        .collect();
    for (mesh_index, mesh) in staged {
        meshes[mesh_index] = mesh;
    }
    Ok(true)
}

fn point_key(point: Point3) -> [u64; 3] {
    point.to_array().map(canonical_coordinate_bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_t_junction_by_splitting_edge_and_moving_vertex() {
        let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let edge_mesh = TriangleMesh::try_new(
            vec![point(0., 0.), point(2., 0.), point(0., -1.)],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let tip_mesh = TriangleMesh::try_new(
            vec![point(1., 0.04), point(1., 1.), point(2., 1.)],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let result = match_mesh_edges(
            &[&edge_mesh, &tip_mesh],
            &MeshEdgeMatchOptions {
                distance: 0.05,
                average: false,
                ratchet: false,
                edge_selection: MeshEdgePick::All,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result.split_edges, 1);
        assert_eq!(result.meshes[0].face_count(), 2);
        assert_eq!(result.meshes[1].vertices()[0], point(1., 0.));
        let a = result.meshes[0].topology_vertex_points();
        let b = result.meshes[1].topology_vertex_points();
        assert!(a.contains(&point(1., 0.)) && b.contains(&point(1., 0.)));
        let averaged = match_mesh_edges(
            &[&edge_mesh, &tip_mesh],
            &MeshEdgeMatchOptions {
                distance: 0.05,
                average: true,
                ratchet: true,
                edge_selection: MeshEdgePick::Shared(vec![0]),
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(averaged.split_edges, 1);
        let midpoint = point(1., 0.02);
        assert!(
            averaged.meshes[0]
                .topology_vertex_points()
                .contains(&midpoint)
        );
        assert!(
            averaged.meshes[1]
                .topology_vertex_points()
                .contains(&midpoint)
        );
        let per_mesh = match_mesh_edges(
            &[&edge_mesh, &tip_mesh],
            &MeshEdgeMatchOptions {
                distance: 0.05,
                average: false,
                ratchet: false,
                edge_selection: MeshEdgePick::PerMesh(vec![(0, 0), (1, 1)]),
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(per_mesh.split_edges, 1);
        assert!(matches!(
            match_mesh_edges(
                &[&edge_mesh],
                &MeshEdgeMatchOptions {
                    distance: 0.05,
                    average: false,
                    ratchet: false,
                    edge_selection: MeshEdgePick::PerMesh(vec![(1, 0)]),
                },
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::MeshSelectionIndexOutOfRange { .. })
        ));
        let unchanged = match_mesh_edges(
            &[&edge_mesh, &tip_mesh],
            &MeshEdgeMatchOptions {
                distance: 0.04,
                average: false,
                ratchet: false,
                edge_selection: MeshEdgePick::All,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(unchanged.split_edges, 0);
        assert_eq!(unchanged.meshes[0], edge_mesh);
        assert_eq!(unchanged.meshes[1], tip_mesh);
    }

    #[test]
    fn repeated_splits_follow_picked_edge_after_topology_renumbering() {
        let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let edge = TriangleMesh::try_new(
            vec![point(0., 0.), point(3., 0.), point(0., -1.)],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let tip = |x, gap| {
            TriangleMesh::try_new(
                vec![point(x, gap), point(x, 1.), point(x + 0.5, 1.)],
                vec![[0, 1, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap()
        };
        let first = tip(1., 0.04);
        let second = tip(2., 0.03);
        let result = match_mesh_edges(
            &[&edge, &first, &second],
            &MeshEdgeMatchOptions {
                distance: 0.05,
                average: false,
                ratchet: false,
                edge_selection: MeshEdgePick::Shared(vec![0]),
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result.split_edges, 2);
        assert_eq!(result.meshes[0].face_count(), 3);
        assert!(
            result.meshes[0]
                .topology_vertex_points()
                .contains(&point(1., 0.))
        );
        assert!(
            result.meshes[0]
                .topology_vertex_points()
                .contains(&point(2., 0.))
        );
        assert_eq!(result.meshes[1].vertices()[0], point(1., 0.));
        assert_eq!(result.meshes[2].vertices()[0], point(2., 0.));
    }

    #[test]
    fn matches_t_junction_within_one_mesh_after_raw_vertex_compaction() {
        let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0., 0.),
                point(2., 0.),
                point(0., -1.),
                point(1., 0.04),
                point(1., 1.),
                point(2., 1.),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_vertex_colors(Some((0..6).map(|value| [value, 20, 30, 0]).collect()))
        .unwrap();
        let result = match_mesh_edges(
            &[&mesh],
            &MeshEdgeMatchOptions {
                distance: 0.05,
                average: true,
                ratchet: false,
                edge_selection: MeshEdgePick::All,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result.split_edges, 1);
        assert_eq!(result.meshes[0].face_count(), 3);
        assert!(
            result.meshes[0]
                .topology_vertex_points()
                .contains(&point(1., 0.02))
        );
        assert_eq!(
            result.meshes[0].vertex_colors().unwrap().len(),
            result.meshes[0].vertices().len()
        );
    }
}
