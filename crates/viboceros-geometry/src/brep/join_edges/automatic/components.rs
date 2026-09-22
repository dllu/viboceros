use super::*;

pub(super) fn collect(
    joined: Brep,
    before: &Brep,
    pairs: &[(usize, usize, bool)],
    sources: &[usize],
    tolerance: Tolerance,
) -> Result<Vec<BrepJoinComponent>, GeometryError> {
    let components = joined.edge_connected_face_components();
    let mut membership = vec![usize::MAX; joined.faces.len()];
    for (component, faces) in components.iter().enumerate() {
        for &face in faces {
            membership[face] = component;
        }
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
