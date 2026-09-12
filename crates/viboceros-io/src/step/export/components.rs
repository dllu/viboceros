//! Partition exported faces by actual shared edges, not coincident positions.
use monstertruck::topology::compress::CompressedShell;
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

fn root(parents: &mut [usize], mut face: usize) -> usize {
    while parents[face] != face {
        parents[face] = parents[parents[face]];
        face = parents[face];
    }
    face
}

pub(super) fn partition<P: Copy, C, S>(
    shell: CompressedShell<P, C, S>,
) -> Vec<CompressedShell<P, C, S>> {
    let mut first_face = vec![None::<usize>; shell.edges.len()];
    let mut parents = (0..shell.faces.len()).collect::<Vec<_>>();
    let mut ranks = vec![0_u8; shell.faces.len()];
    for (face, item) in shell.faces.iter().enumerate() {
        for edge in item.boundaries.iter().flatten() {
            if let Some(first) = first_face[edge.index] {
                let mut left = root(&mut parents, first);
                let mut right = root(&mut parents, face);
                if left != right {
                    if ranks[left] < ranks[right] {
                        std::mem::swap(&mut left, &mut right);
                    }
                    parents[right] = left;
                    if ranks[left] == ranks[right] {
                        ranks[left] += 1;
                    }
                }
            } else {
                first_face[edge.index] = Some(face);
            }
        }
    }
    let mut component = vec![usize::MAX; shell.faces.len()];
    let mut component_by_root = vec![usize::MAX; shell.faces.len()];
    let mut count = 0;
    for (face, group) in component.iter_mut().enumerate() {
        let root = root(&mut parents, face);
        if component_by_root[root] == usize::MAX {
            component_by_root[root] = count;
            count += 1;
        }
        *group = component_by_root[root];
    }
    if count == 1 {
        return vec![shell];
    }
    let mut pieces = (0..count)
        .map(|_| CompressedShell {
            vertices: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
            vertex_stable_ids: None,
            edge_stable_ids: None,
            face_stable_ids: None,
        })
        .collect::<Vec<_>>();
    let mut vertex_maps = vec![BTreeMap::new(); count];
    let mut edge_map = vec![usize::MAX; shell.edges.len()];
    let mut edges = shell.edges.into_iter().map(Some).collect::<Vec<_>>();
    // Preserve source face order within each component. An edge belongs to
    // exactly one component; move its curve rather than cloning it. Vertices
    // may belong to several edge-disconnected components and are copied.
    for (face, mut item) in shell.faces.into_iter().enumerate() {
        let group = component[face];
        let piece = &mut pieces[group];
        for edge_use in item.boundaries.iter_mut().flatten() {
            let source = edge_use.index;
            if edge_map[source] == usize::MAX {
                let mut edge = edges[source].take().expect("edge belongs to one component");
                let mut remap = |raw| {
                    *vertex_maps[group].entry(raw).or_insert_with(|| {
                        let local = piece.vertices.len();
                        piece.vertices.push(shell.vertices[raw]);
                        local
                    })
                };
                edge.vertices = (remap(edge.vertices.0), remap(edge.vertices.1));
                edge_map[source] = piece.edges.len();
                piece.edges.push(edge);
            }
            edge_use.index = edge_map[source];
        }
        piece.faces.push(item);
    }
    pieces
}
