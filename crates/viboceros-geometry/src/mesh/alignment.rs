//! Align nearby naked mesh vertices while preserving each mesh's raw topology.
use super::*;

/// Scriptable subset of topology components eligible to move.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MeshAlignSelection {
    AllNaked,
    Vertices(Vec<usize>),
    NakedEdges(Vec<usize>),
}

struct Source {
    data: MeshTopologyData,
    selected: Vec<bool>,
    naked: Vec<bool>,
    faces_by_vertex: Vec<Vec<usize>>,
    raw_by_vertex: Vec<Vec<usize>>,
}

#[derive(Clone, Copy)]
struct Candidate {
    mesh: usize,
    topology: usize,
    point: Point3,
    selected: bool,
}

struct KdNode {
    candidate: usize,
    axis: usize,
    left: Option<usize>,
    right: Option<usize>,
}

/// Aligns naked vertices across all input meshes. Each topology vertex joins
/// at most one nearest pair, and every move is strictly shorter than `distance`.
/// Unselected vertices can anchor selected vertices but never move themselves.
/// When averaging, two selected vertices move to their midpoint.
/// Meshes with an invalid resulting face are rejected together.
pub fn align_mesh_vertices(
    meshes: &[&TriangleMesh],
    distance: Real,
    average: bool,
    selection: &MeshAlignSelection,
    tolerance: Tolerance,
) -> Result<Vec<(TriangleMesh, usize)>, GeometryError> {
    align_mesh_vertices_impl(meshes, distance, average, Some(selection), None, tolerance)
}

pub(super) fn align_mesh_vertices_with_masks(
    meshes: &[&TriangleMesh],
    distance: Real,
    average: bool,
    masks: &[Vec<bool>],
    tolerance: Tolerance,
) -> Result<Vec<(TriangleMesh, usize)>, GeometryError> {
    debug_assert_eq!(meshes.len(), masks.len());
    align_mesh_vertices_impl(meshes, distance, average, None, Some(masks), tolerance)
}

fn align_mesh_vertices_impl(
    meshes: &[&TriangleMesh],
    distance: Real,
    average: bool,
    selection: Option<&MeshAlignSelection>,
    masks: Option<&[Vec<bool>]>,
    tolerance: Tolerance,
) -> Result<Vec<(TriangleMesh, usize)>, GeometryError> {
    if !distance.is_finite() || distance <= 0. {
        return Err(GeometryError::InvalidTolerance);
    }
    let sources = meshes
        .iter()
        .enumerate()
        .map(|(index, mesh)| prepare_source(mesh, selection, masks.map(|masks| &masks[index][..])))
        .collect::<Result<Vec<_>, _>>()?;
    let mut candidates = Vec::new();
    for (mesh, source) in sources.iter().enumerate() {
        for topology in 0..source.data.topological_vertex_count {
            if source.naked[topology] {
                candidates.push(Candidate {
                    mesh,
                    topology,
                    point: source.data.topological_points[topology],
                    selected: source.selected[topology],
                });
            }
        }
    }
    let mut indices = (0..candidates.len()).collect::<Vec<_>>();
    let mut tree = Vec::with_capacity(candidates.len());
    let root = build_kd_tree(&mut indices, &candidates, &mut tree);
    let mut proposals = BTreeMap::new();
    for (a, candidate) in candidates.iter().enumerate() {
        if !candidate.selected {
            continue;
        }
        let mut nearest = None;
        if let Some(root) = root {
            nearest_kd_neighbor(
                a,
                root,
                &tree,
                &candidates,
                &sources,
                distance,
                &mut nearest,
            );
        }
        if let Some((separation, b)) = nearest {
            proposals.insert((a.min(b), a.max(b)), separation);
        }
    }
    let mut proposals = proposals.into_iter().collect::<Vec<_>>();
    proposals.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut used = vec![false; candidates.len()];
    let mut vertices = meshes
        .iter()
        .map(|mesh| mesh.vertices.clone())
        .collect::<Vec<_>>();
    let mut moved = vec![0; meshes.len()];
    for ((a, b), _) in proposals {
        if used[a] || used[b] {
            continue;
        }
        used[a] = true;
        used[b] = true;
        let first = candidates[a];
        let second = candidates[b];
        let midpoint = if average && first.selected && second.selected {
            Some(first.point.midpoint(second.point)?)
        } else {
            None
        };
        let first_target = midpoint.unwrap_or({
            if second.selected {
                first.point
            } else {
                second.point
            }
        });
        let second_target = midpoint.unwrap_or(first.point);
        for (candidate, target) in [(first, first_target), (second, second_target)] {
            if !candidate.selected || candidate.point == target {
                continue;
            }
            for &raw in &sources[candidate.mesh].raw_by_vertex[candidate.topology] {
                vertices[candidate.mesh][raw] = target;
                moved[candidate.mesh] += 1;
            }
        }
    }
    meshes
        .iter()
        .enumerate()
        .map(|(index, mesh)| {
            if moved[index] == 0 {
                return Ok(((*mesh).clone(), 0));
            }
            let rebuilt = TriangleMesh::try_new_faces(
                std::mem::take(&mut vertices[index]),
                mesh.faces.clone(),
                tolerance,
            )?
            .try_with_vertex_colors(mesh.vertex_colors.clone())?
            .try_with_ngons(mesh.ngons.clone())?;
            Ok((rebuilt, moved[index]))
        })
        .collect()
}

fn build_kd_tree(
    indices: &mut [usize],
    candidates: &[Candidate],
    nodes: &mut Vec<KdNode>,
) -> Option<usize> {
    if indices.is_empty() {
        return None;
    }
    let mut minimum = [Real::INFINITY; 3];
    let mut maximum = [Real::NEG_INFINITY; 3];
    for &index in indices.iter() {
        for axis in 0..3 {
            let value = candidates[index].point.to_array()[axis];
            minimum[axis] = minimum[axis].min(value);
            maximum[axis] = maximum[axis].max(value);
        }
    }
    let axis = (0..3)
        .max_by(|&a, &b| (maximum[a] - minimum[a]).total_cmp(&(maximum[b] - minimum[b])))
        .unwrap();
    let middle = indices.len() / 2;
    indices.select_nth_unstable_by(middle, |&a, &b| {
        candidates[a].point.to_array()[axis]
            .total_cmp(&candidates[b].point.to_array()[axis])
            .then_with(|| a.cmp(&b))
    });
    let candidate = indices[middle];
    let node_index = nodes.len();
    nodes.push(KdNode {
        candidate,
        axis,
        left: None,
        right: None,
    });
    let (left, remaining) = indices.split_at_mut(middle);
    let right = &mut remaining[1..];
    let left = build_kd_tree(left, candidates, nodes);
    let right = build_kd_tree(right, candidates, nodes);
    nodes[node_index].left = left;
    nodes[node_index].right = right;
    Some(node_index)
}

fn nearest_kd_neighbor(
    query: usize,
    node_index: usize,
    nodes: &[KdNode],
    candidates: &[Candidate],
    sources: &[Source],
    limit: Real,
    best: &mut Option<(Real, usize)>,
) {
    let node = &nodes[node_index];
    let pivot = node.candidate;
    let difference = candidates[query].point.to_array()[node.axis]
        - candidates[pivot].point.to_array()[node.axis];
    let (near, far) = if difference <= 0. {
        (node.left, node.right)
    } else {
        (node.right, node.left)
    };
    if let Some(near) = near {
        nearest_kd_neighbor(query, near, nodes, candidates, sources, limit, best);
    }
    if query != pivot
        && !(candidates[query].mesh == candidates[pivot].mesh
            && share_face(
                &sources[candidates[query].mesh],
                candidates[query].topology,
                candidates[pivot].topology,
            ))
        && let Ok(separation) = candidates[query].point.distance_to(candidates[pivot].point)
        && separation > 0.
        && separation < limit
        && best.is_none_or(|(prior, partner)| {
            separation < prior || separation == prior && pivot < partner
        })
    {
        *best = Some((separation, pivot));
    }
    if difference.abs() <= best.map_or(limit, |(separation, _)| separation)
        && let Some(far) = far
    {
        nearest_kd_neighbor(query, far, nodes, candidates, sources, limit, best);
    }
}

fn prepare_source(
    mesh: &TriangleMesh,
    selection: Option<&MeshAlignSelection>,
    raw_mask: Option<&[bool]>,
) -> Result<Source, GeometryError> {
    let data = mesh.topology_data();
    let count = data.topological_vertex_count;
    let mut selected = vec![false; count];
    if let Some(raw_mask) = raw_mask {
        debug_assert_eq!(raw_mask.len(), mesh.vertices.len());
        for (raw, &enabled) in raw_mask.iter().enumerate() {
            if enabled {
                selected[data.topological_vertices[raw]] = true;
            }
        }
    } else {
        match selection.expect("either a selection or raw masks are provided") {
            MeshAlignSelection::AllNaked => selected.fill(true),
            MeshAlignSelection::Vertices(indices) => {
                for &vertex in indices {
                    if vertex >= count {
                        return Err(GeometryError::MeshTopologyVertexIndexOutOfRange {
                            vertex,
                            vertex_count: count,
                        });
                    }
                    selected[vertex] = true;
                }
            }
            MeshAlignSelection::NakedEdges(indices) => {
                for &edge in indices {
                    let Some((&(a, b), incidence)) = data.edges.iter().nth(edge) else {
                        return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                            edge,
                            edge_count: data.edges.len(),
                        });
                    };
                    if incidence.count == 1 {
                        selected[a] = true;
                        selected[b] = true;
                    }
                }
            }
        }
    }
    let mut raw_by_vertex = vec![Vec::new(); count];
    for (raw, &topology) in data.topological_vertices.iter().enumerate() {
        raw_by_vertex[topology].push(raw);
    }
    let mut faces_by_vertex = vec![Vec::new(); count];
    let mut naked = vec![false; count];
    for (&(a, b), incidence) in &data.edges {
        if incidence.count == 1 {
            naked[a] = true;
            naked[b] = true;
        }
    }
    for (face_index, face) in mesh.faces.iter().enumerate() {
        for &raw in face.indices() {
            let topology = data.topological_vertices[raw as usize];
            if faces_by_vertex[topology].last() != Some(&face_index) {
                faces_by_vertex[topology].push(face_index);
            }
        }
    }
    Ok(Source {
        data,
        selected,
        naked,
        faces_by_vertex,
        raw_by_vertex,
    })
}

fn share_face(source: &Source, a: usize, b: usize) -> bool {
    source.faces_by_vertex[a]
        .iter()
        .any(|face| source.faces_by_vertex[b].binary_search(face).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_triangle(x: Real) -> TriangleMesh {
        TriangleMesh::try_new(
            vec![
                Point3::try_new(x, 0., 0.).unwrap(),
                Point3::try_new(x + 1., 0., 0.).unwrap(),
                Point3::try_new(x, 1., 0.).unwrap(),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn aligns_near_naked_vertices_across_meshes_and_respects_masks() {
        let left = open_triangle(0.)
            .try_with_vertex_colors(Some(vec![[1, 2, 3, 0], [4, 5, 6, 0], [7, 8, 9, 0]]))
            .unwrap();
        let right = open_triangle(1.04);
        let result = align_mesh_vertices(
            &[&left, &right],
            0.05,
            false,
            &MeshAlignSelection::AllNaked,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result[0].1, 0);
        assert_eq!(result[1].1, 1);
        assert_eq!(result[1].0.vertices()[0], left.vertices()[1]);
        let result = align_mesh_vertices(
            &[&left, &right],
            0.05,
            true,
            &MeshAlignSelection::AllNaked,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result[0].1, 1);
        assert_eq!(result[1].1, 1);
        assert_eq!(result[0].0.vertices()[1], result[1].0.vertices()[0]);
        assert_eq!(result[0].0.vertex_colors(), left.vertex_colors());
        let result = align_mesh_vertices(
            &[&left, &right],
            0.05,
            true,
            &MeshAlignSelection::Vertices(vec![1]),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result[0].1, 1);
        assert_eq!(result[1].1, 0);
        assert_eq!(result[0].0.vertices()[1], result[1].0.vertices()[0]);
        let result = align_mesh_vertices(
            &[&left, &right],
            0.05,
            true,
            &MeshAlignSelection::NakedEdges(vec![0]),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!((result[0].1, result[1].1), (1, 1));
        assert!(matches!(
            align_mesh_vertices(
                &[&left],
                0.05,
                false,
                &MeshAlignSelection::NakedEdges(vec![3]),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::MeshTopologyEdgeIndexOutOfRange { .. })
        ));
    }

    #[test]
    fn strict_threshold_and_same_face_protection() {
        let left = open_triangle(0.);
        let right = open_triangle(1.05);
        let result = align_mesh_vertices(
            &[&left, &right],
            0.05,
            false,
            &MeshAlignSelection::AllNaked,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result.iter().map(|item| item.1).sum::<usize>(), 0);
        let short = open_triangle(0.);
        let result = align_mesh_vertices(
            &[&short],
            1.1,
            false,
            &MeshAlignSelection::AllNaked,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result[0].1, 0);
    }

    #[test]
    fn already_coincident_naked_vertices_do_not_consume_a_near_pair() {
        let left = open_triangle(0.);
        let coincident = open_triangle(0.);
        let near = open_triangle(0.04);
        let result = align_mesh_vertices(
            &[&left, &coincident, &near],
            0.05,
            false,
            &MeshAlignSelection::AllNaked,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result[0].1 + result[1].1 + result[2].1, 3);
    }

    #[test]
    fn planar_triangle_soups_match_many_independent_near_vertices() {
        let make = |offset: Real| {
            let mut vertices = Vec::new();
            let mut triangles = Vec::new();
            for row in 0..128 {
                let base = vertices.len() as u32;
                let y = row as Real * 10.;
                vertices.extend([
                    Point3::try_new(offset, y, 0.).unwrap(),
                    Point3::try_new(offset + 1., y, 0.).unwrap(),
                    Point3::try_new(offset, y + 1., 0.).unwrap(),
                ]);
                triangles.push([base, base + 1, base + 2]);
            }
            TriangleMesh::try_new(vertices, triangles, Tolerance::DEFAULT).unwrap()
        };
        let left = make(0.);
        let right = make(1.04);
        let aligned = align_mesh_vertices(
            &[&left, &right],
            0.05,
            false,
            &MeshAlignSelection::AllNaked,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(aligned[0].1, 0);
        assert_eq!(aligned[1].1, 128);
    }
}
