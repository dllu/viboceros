//! Topology-edge collapse, seam-preserving endpoint merges, and face reduction.
use super::*;

#[cfg(test)]
mod tests;

/// Remove index-degenerate faces and rotate a singly collapsed quad side to
/// the triangle order used by CollapseEdge. Geometry is validated afterwards.
fn reduced_face(face: MeshFace) -> Option<MeshFace> {
    match face {
        MeshFace::Triangle([a, b, c]) => (a != b && b != c && c != a).then_some(face),
        MeshFace::Quad(indices) => {
            let mut collapsed = (0..4).filter(|&side| indices[side] == indices[(side + 1) % 4]);
            match (collapsed.next(), collapsed.next()) {
                (None, None) => {
                    (indices[0] != indices[2] && indices[1] != indices[3]).then_some(face)
                }
                (Some(side), None) => {
                    let start = (side + 2) % 4;
                    reduced_face(MeshFace::Triangle([
                        indices[start],
                        indices[(start + 1) % 4],
                        indices[(start + 2) % 4],
                    ]))
                }
                _ => None,
            }
        }
    }
}

impl TriangleMesh {
    /// Replaces one exact-location topology edge with vertices at its center.
    ///
    /// Every raw vertex belonging to either topology endpoint moves to the
    /// midpoint. Raw endpoint pairs used by the selected edge are merged, so
    /// welded and partially welded uses stay joined while independent seam
    /// components remain distinct. Collapsed triangle faces disappear;
    /// collapsed quad sides become triangles. Surviving faces retain source
    /// order and referenced vertices compact in source order, matching
    /// RhinoCommon's `MeshTopologyEdgeList::CollapseEdge` behavior. `None`
    /// means that the collapse removes every face.
    pub fn collapse_topology_edge(
        &self,
        edge_index: usize,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let data = self.topology_data();
        let edge_count = data.edges.len();
        let Some((&(first_topology_vertex, second_topology_vertex), incidence)) =
            data.edges.iter().nth(edge_index)
        else {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: edge_index,
                edge_count,
            });
        };
        let first = data.topological_points[first_topology_vertex];
        let second = data.topological_points[second_topology_vertex];
        let midpoint = first.midpoint(second)?;

        let mut parents = (0..self.vertices.len()).collect::<Vec<_>>();
        for edge_use in incidence.uses() {
            union_indices_keep_earlier(
                &mut parents,
                edge_use.raw_vertices[0] as usize,
                edge_use.raw_vertices[1] as usize,
            );
        }
        let mut faces = Vec::with_capacity(self.faces.len());
        for face in self.faces.iter().copied() {
            let remapped = face.remapped(|raw| {
                u32::try_from(index_root(&mut parents, raw as usize))
                    .expect("a mesh raw vertex index already fits in u32")
            });
            if let Some(face) = reduced_face(remapped) {
                faces.push(face);
            }
        }
        if faces.is_empty() {
            return Ok(None);
        }

        let mut used = vec![false; self.vertices.len()];
        for face in &faces {
            for &raw in face.indices() {
                used[raw as usize] = true;
            }
        }
        let retained_vertex_count = used.iter().filter(|&&retain| retain).count();
        let mut raw_remap = vec![0_u32; self.vertices.len()];
        let mut vertices = Vec::with_capacity(retained_vertex_count);
        for (raw, (&point, retain)) in self.vertices.iter().zip(used).enumerate() {
            if !retain {
                continue;
            }
            raw_remap[raw] =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            let topology_vertex = data.topological_vertices[raw];
            vertices.push(
                if topology_vertex == first_topology_vertex
                    || topology_vertex == second_topology_vertex
                {
                    midpoint
                } else {
                    point
                },
            );
        }
        let faces = faces
            .into_iter()
            .map(|face| face.remapped(|raw| raw_remap[raw as usize]))
            .collect();
        Ok(Some(Self::try_new_faces(vertices, faces, tolerance)?))
    }
}
