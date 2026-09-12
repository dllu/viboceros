//! Radial mesh edge, face-walk, and component ordering policies.
//!
//! Face walks retain first face occurrences before grouping. Component order
//! instead removes a repeated cyclic root after grouping, which can rotate the
//! output order. These two traversals must not be collapsed into one dedup pass.
use super::{EdgeIncidence, index_root};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

/// Reproduces `ON_MeshTopology::SortVertexEdges`: each returned group is one
/// radial fan, starting at a naked/non-manifold edge when one is present.
/// Incident edge indices must be strictly increasing, as produced by topology
/// map traversal. Priority lists retain this order without removing entries.
pub(super) fn radially_sorted_vertex_edges(
    topological_vertex: usize,
    incident_edges: &[usize],
    edges: &[([usize; 2], &EdgeIncidence)],
    face_edges: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    debug_assert!(incident_edges.windows(2).all(|pair| pair[0] < pair[1]));
    let mut naked = Vec::new();
    let mut manifold = Vec::new();
    let mut non_manifold = Vec::new();
    for (local, &edge) in incident_edges.iter().enumerate() {
        let (vertices, incidence) = edges[edge];
        debug_assert!(vertices.contains(&topological_vertex));
        match incidence.count {
            1 => naked.push(local),
            2 => manifold.push(local),
            _ => non_manifold.push(local),
        }
    }
    let mut pending = vec![true; incident_edges.len()];

    let mut groups = Vec::new();
    for local in naked.into_iter().chain(non_manifold).chain(manifold) {
        if !pending[local] {
            continue;
        }
        pending[local] = false;
        let first = incident_edges[local];
        let mut group = vec![first];
        let mut current = first;
        let mut group_direction = 0_i8;
        loop {
            let (vertices, incidence) = edges[current];
            let mut next = None;
            for edge_use in incidence.uses() {
                let direction = if vertices[0] == topological_vertex {
                    if edge_use.forward { -1 } else { 1 }
                } else if edge_use.forward {
                    1
                } else {
                    -1
                };
                let side_count = face_edges[edge_use.face].len();
                let next_side = if direction < 0 {
                    (edge_use.side + side_count - 1) % side_count
                } else {
                    (edge_use.side + 1) % side_count
                };
                let candidate = face_edges[edge_use.face][next_side];
                let removed = incident_edges.binary_search(&candidate).is_ok_and(|local| {
                    let was_pending = pending[local];
                    pending[local] = false;
                    was_pending
                });
                if removed {
                    if group_direction == 0 {
                        group_direction = direction;
                    }
                    next = Some(candidate);
                    break;
                }
            }
            let Some(next_edge) = next else {
                break;
            };
            group.push(next_edge);
            current = next_edge;
        }
        if group_direction > 0 {
            group.reverse();
        }
        groups.push(group);
    }
    groups
}

/// Topology construction appends uses in ascending face-index order. Merging
/// the two streams finds the first shared face without a Cartesian scan.
fn shared_edge_face(first: &EdgeIncidence, second: &EdgeIncidence) -> Option<usize> {
    let mut first = first.uses();
    let mut second = second.uses();
    let mut left = first.next()?.face;
    let mut right = second.next()?.face;
    loop {
        if left == right {
            return Some(left);
        }
        if left < right {
            left = first.next()?.face;
        } else {
            right = second.next()?.face;
        }
    }
}

/// First occurrences in radial traversal order, with each incoming edge.
/// Singleton groups carry faces too; falling back to source face order for
/// those groups loses Rhino's non-manifold component ordering.
/// Incident face indices must be strictly increasing, as in topology traversal.
pub(super) fn radial_vertex_face_walk(
    edge_groups: &[Vec<usize>],
    edges: &[([usize; 2], &EdgeIncidence)],
    incident_faces: &[usize],
) -> Vec<(usize, Option<usize>)> {
    debug_assert!(incident_faces.windows(2).all(|pair| pair[0] < pair[1]));
    let mut walk = Vec::new();
    let mut seen = vec![false; incident_faces.len()];
    for group in edge_groups {
        let mut faces = Vec::new();
        if group.len() == 1 {
            faces.extend(
                edges[group[0]]
                    .1
                    .uses()
                    .map(|edge_use| (edge_use.face, Some(group[0]))),
            );
        } else {
            for (first, second) in group
                .iter()
                .copied()
                .zip(group.iter().copied().cycle().skip(1))
                .take(group.len())
            {
                if let Some(face) = shared_edge_face(edges[first].1, edges[second].1)
                    && faces.last().is_none_or(|&(previous, _)| previous != face)
                {
                    faces.push((face, Some(first)));
                }
            }
            if faces.len() > 1 && faces.first().unwrap().0 == faces.last().unwrap().0 {
                faces.remove(0);
            }
        }
        for entry in faces {
            let local = incident_faces
                .binary_search(&entry.0)
                .expect("a radial face belongs to the incident face list");
            if !std::mem::replace(&mut seen[local], true) {
                walk.push(entry);
            }
        }
    }
    for (local, &face) in incident_faces.iter().enumerate() {
        if !seen[local] {
            walk.push((face, None));
        }
    }
    walk
}

pub(super) fn ordered_vertex_face_components(
    edge_groups: &[Vec<usize>],
    edges: &[([usize; 2], &EdgeIncidence)],
    face_to_local: &BTreeMap<usize, usize>,
    incident_faces: &[usize],
    parents: &mut [usize],
) -> Vec<usize> {
    let mut order = Vec::new();
    let mut seen = vec![false; parents.len()];
    for edge_group in edge_groups {
        let mut radial_roots = Vec::new();
        if edge_group.len() == 1 {
            for edge_use in edges[edge_group[0]].1.uses() {
                if let Some(&local) = face_to_local.get(&edge_use.face) {
                    radial_roots.push(index_root(parents, local));
                }
            }
        }
        let mut add_shared_root = |first: usize, second: usize| {
            let Some(face) = shared_edge_face(edges[first].1, edges[second].1) else {
                return;
            };
            let Some(&local) = face_to_local.get(&face) else {
                return;
            };
            let root = index_root(parents, local);
            if radial_roots.last().copied() != Some(root) {
                radial_roots.push(root);
            }
        };
        for pair in edge_group.windows(2) {
            add_shared_root(pair[0], pair[1]);
        }
        if edge_group.len() > 1 {
            add_shared_root(*edge_group.last().unwrap(), edge_group[0]);
        }
        if radial_roots.len() > 1 && radial_roots.first() == radial_roots.last() {
            radial_roots.remove(0);
        }
        for root in radial_roots {
            if !std::mem::replace(&mut seen[root], true) {
                order.push(root);
            }
        }
    }
    for &face in incident_faces {
        let root = index_root(parents, face_to_local[&face]);
        if !std::mem::replace(&mut seen[root], true) {
            order.push(root);
        }
    }
    order
}
