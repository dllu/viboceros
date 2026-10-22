//! Selected topology-edge welding retains the first incident face's endpoints.
use super::weld::attributes::normalized_sum;
use super::{GeometryError, TriangleMesh, index_root};
use std::borrow::Cow;
use std::collections::BTreeSet;

#[cfg(test)]
mod tests;

impl TriangleMesh {
    /// Welds coincident raw endpoint sets along selected exact-location
    /// topology edges.
    ///
    /// Indices use the same deterministic order as [`Self::wireframe_lines`].
    /// Only raw vertices used by faces incident to a selected edge are merged;
    /// other coincident fan components remain separate. The first incident
    /// face's raw vertex survives, and a non-empty valid selection compacts unused
    /// vertices. The returned count is the number of selected edges that had
    /// at least one divided endpoint set.
    pub fn welded_topology_edges(
        &self,
        edge_indices: &[usize],
    ) -> Result<(Self, usize), GeometryError> {
        self.welded_topology_edges_with_normal_generation(edge_indices, false)
    }

    /// As [`Self::welded_topology_edges`], optionally generating missing source
    /// normals as the native document command does. Existing normals are
    /// averaged only at merged endpoints; UVs and colors keep their survivor.
    pub fn welded_topology_edges_with_normal_generation(
        &self,
        edge_indices: &[usize],
        generate_normals: bool,
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
        let mut normals = if generate_normals {
            self.stored_or_computed_normals()?
        } else {
            self.vertex_normals().map(Cow::Borrowed)
        };
        let mut welded_edge_count = 0;
        for (incidence, selected) in data.edges.values().zip(selected_edges) {
            if !selected {
                continue;
            }
            let mut uses = incidence.uses();
            let first = uses.next().expect("a topology edge has an incident face");
            let keep = first
                .raw_vertices
                .map(|raw| index_root(&mut parents, raw as usize));
            let mut divided_endpoint = false;
            for endpoint in 0..2 {
                let roots = incidence
                    .uses()
                    .map(|u| index_root(&mut parents, u.raw_vertices[endpoint] as usize))
                    .collect::<BTreeSet<_>>();
                if roots.len() > 1
                    && let Some(normals) = &mut normals
                {
                    // Each face use contributes, including repeated uses of
                    // an already shared raw endpoint on a non-manifold edge.
                    let normal = normalized_sum(incidence.uses().map(|u| {
                        normals[index_root(&mut parents, u.raw_vertices[endpoint] as usize)]
                    }))?
                    .unwrap_or(super::Vector3::try_from([0.; 3])?);
                    normals.to_mut()[keep[endpoint]] = normal;
                }
            }
            for edge_use in uses {
                for (endpoint, &target) in keep.iter().enumerate() {
                    let first = first.raw_vertices[endpoint] as usize;
                    let raw = edge_use.raw_vertices[endpoint] as usize;
                    // The first incident face supplies the representative,
                    // including when its raw index follows the other faces.
                    if first != raw {
                        divided_endpoint = true;
                        let root = index_root(&mut parents, raw);
                        parents[root] = target;
                    }
                }
            }
            welded_edge_count += usize::from(divided_endpoint);
        }
        let (welded, _) =
            self.compacted_with_vertex_parents_and_normals(parents, normals.as_deref());
        Ok((welded, welded_edge_count))
    }
}
