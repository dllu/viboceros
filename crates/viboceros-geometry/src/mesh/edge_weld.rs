//! Selected topology-edge welding with earliest-index survivor policy.
use super::{GeometryError, TriangleMesh, union_indices_keep_earlier};

#[cfg(test)]
mod tests;

impl TriangleMesh {
    /// Welds coincident raw endpoint sets along selected exact-location
    /// topology edges.
    ///
    /// Indices use the same deterministic order as [`Self::wireframe_lines`].
    /// Only raw vertices used by faces incident to a selected edge are merged;
    /// other coincident fan components remain separate. The earliest source
    /// raw vertex survives, and a non-empty valid selection compacts unused
    /// vertices. The returned count is the number of selected edges that had
    /// at least one divided endpoint set.
    pub fn welded_topology_edges(
        &self,
        edge_indices: &[usize],
    ) -> Result<(Self, usize), GeometryError> {
        if edge_indices.is_empty() {
            return Ok((self.clone(), 0));
        }
        let data = self.topology_data();
        let edge_count = data.edges.len();
        if let Some(&edge) = edge_indices.iter().find(|&&edge| edge >= edge_count) {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange { edge, edge_count });
        }
        let mut selected_edges = vec![false; edge_count];
        for &edge in edge_indices {
            selected_edges[edge] = true;
        }

        let mut parents = (0..self.vertices.len()).collect::<Vec<_>>();
        let mut welded_edge_count = 0;
        for (incidence, selected) in data.edges.values().zip(selected_edges) {
            if !selected {
                continue;
            }
            let mut uses = incidence.uses();
            let first = uses.next().expect("a topology edge has an incident face");
            let mut divided_endpoint = false;
            for edge_use in uses {
                for endpoint in 0..2 {
                    let first = first.raw_vertices[endpoint] as usize;
                    let raw = edge_use.raw_vertices[endpoint] as usize;
                    // Index-preserving unions retain the minimum raw index
                    // independently of traversal order or repeated uses.
                    if first != raw {
                        divided_endpoint = true;
                        union_indices_keep_earlier(&mut parents, first, raw);
                    }
                }
            }
            welded_edge_count += usize::from(divided_endpoint);
        }
        let (welded, _) = self.compacted_with_vertex_parents(&mut parents);
        Ok((welded, welded_edge_count))
    }
}
