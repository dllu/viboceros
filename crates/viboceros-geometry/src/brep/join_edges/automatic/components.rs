use super::*;

pub(super) fn collect(
    joined: Brep,
    before: &Brep,
    pairs: &[(usize, usize, bool)],
    sources: &[usize],
    tolerance: Tolerance,
) -> Result<Vec<BrepJoinComponent>, GeometryError> {
    let mut first: Vec<Option<usize>> = vec![None; joined.edges.len()];
    let mut adjacent = vec![Vec::new(); joined.faces.len()];
    for usage in joined.trim_uses() {
        if let Some(edge) = usage.trim.edge {
            if let Some(face) = first[edge] {
                adjacent[usage.face].push(face);
                adjacent[face].push(usage.face);
            } else {
                first[edge] = Some(usage.face);
            }
        }
    }
    let mut membership = vec![usize::MAX; joined.faces.len()];
    let mut components = Vec::new();
    for seed in 0..joined.faces.len() {
        if membership[seed] != usize::MAX {
            continue;
        }
        let mut pending = vec![seed];
        let mut faces = Vec::new();
        membership[seed] = components.len();
        while let Some(face) = pending.pop() {
            faces.push(face);
            for &next in &adjacent[face] {
                if membership[next] == usize::MAX {
                    membership[next] = components.len();
                    pending.push(next);
                }
            }
        }
        faces.sort_unstable();
        components.push(faces);
    }
    let mut counts = vec![0; components.len()];
    let mut edge_face = vec![0; before.edges.len()];
    for usage in before.trim_uses() {
        if let Some(e) = usage.trim.edge {
            edge_face[e] = usage.face;
        }
    }
    for &(a, _, _) in pairs {
        counts[membership[edge_face[a]]] += 1;
    }
    let mut outputs = Vec::with_capacity(components.len());
    let mut vertex_map = vec![0; joined.vertices.len()];
    let mut edge_map = vec![0; joined.edges.len()];
    for (i, faces) in components.into_iter().enumerate() {
        let source_indices = faces
            .iter()
            .map(|&f| sources[f])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        // Gather only this component's references. Repeated sub_brep calls
        // would rescan every global vertex/edge for every isolated component.
        let mut vertex_ids = Vec::new();
        let mut edge_ids = Vec::new();
        for &f in &faces {
            for trim in joined.faces[f].loops.iter().flat_map(|l| &l.trims) {
                vertex_ids.extend(trim.vertices);
                if let Some(e) = trim.edge {
                    edge_ids.push(e);
                }
            }
        }
        vertex_ids.sort_unstable();
        vertex_ids.dedup();
        edge_ids.sort_unstable();
        edge_ids.dedup();
        let vertices = vertex_ids
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                vertex_map[v] = i;
                joined.vertices[v]
            })
            .collect();
        let edges = edge_ids
            .into_iter()
            .enumerate()
            .map(|(i, e)| {
                edge_map[e] = i;
                let mut edge = joined.edges[e].clone();
                edge.vertices = edge.vertices.map(|v| vertex_map[v]);
                edge
            })
            .collect();
        let faces = faces
            .into_iter()
            .map(|f| {
                let mut face = joined.faces[f].clone();
                for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
                    trim.vertices = trim.vertices.map(|v| vertex_map[v]);
                    trim.edge = trim.edge.map(|e| edge_map[e]);
                }
                face
            })
            .collect();
        let mut brep = Brep::try_new(vertices, edges, faces, tolerance)?;
        if counts[i] > 0 && brep.is_solid() && brep.signed_volume(tolerance)? < 0. {
            brep = brep.reversed();
        }
        outputs.push(BrepJoinComponent {
            brep,
            source_indices,
            joined_edge_count: counts[i],
        });
    }
    Ok(outputs)
}
