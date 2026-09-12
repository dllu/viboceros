//! Face-local separation at selected exact-location topology vertices.
use std::collections::BTreeSet;

use super::{
    GeometryError, TriangleMesh, radial_vertex_face_walk, radially_sorted_vertex_edges,
    topology_face_edge_indices,
};

#[cfg(test)]
mod tests;

impl TriangleMesh {
    /// Gives every face incident to the supplied exact-location topology
    /// vertices its own raw mesh vertex.
    ///
    /// Indices use the same deterministic order as
    /// [`Self::topology_vertex_points`]. A non-empty valid selection compacts
    /// unused vertices, and selected vertices with multiple incident faces are
    /// rebuilt in OpenNURBS radial order even when they were already unwelded.
    /// The returned count is the number of selected topology vertices that
    /// required at least one new face-local raw vertex.
    pub fn unwelded_topology_vertices(
        &self,
        vertex_indices: &[usize],
    ) -> Result<(Self, usize), GeometryError> {
        if vertex_indices.is_empty() {
            return Ok((self.clone(), 0));
        }
        let data = self.topology_data();
        let vertex_count = data.topological_vertex_count;
        if let Some(&vertex) = vertex_indices
            .iter()
            .find(|&&vertex| vertex >= vertex_count)
        {
            return Err(GeometryError::MeshTopologyVertexIndexOutOfRange {
                vertex,
                vertex_count,
            });
        }
        let mut selected_vertices = vec![false; vertex_count];
        for &vertex in vertex_indices {
            selected_vertices[vertex] = true;
        }

        let mut incident_faces = vec![Vec::new(); data.topological_vertex_count];
        for (face, polygon) in self.faces.iter().enumerate() {
            for &raw in polygon.indices() {
                let topological_vertex = data.topological_vertices[raw as usize];
                if selected_vertices[topological_vertex]
                    && incident_faces[topological_vertex].last().copied() != Some(face)
                {
                    incident_faces[topological_vertex].push(face);
                }
            }
        }
        let affected_vertices = selected_vertices
            .iter()
            .zip(&incident_faces)
            .map(|(&selected, faces)| selected && faces.len() > 1)
            .collect::<Vec<_>>();
        if !affected_vertices.iter().any(|&affected| affected) {
            return Ok((self.culled_unused_vertices().0, 0));
        }

        let mut newly_separated_vertex_count = 0;
        for (topological_vertex, faces) in incident_faces.iter().enumerate() {
            if !affected_vertices[topological_vertex] {
                continue;
            }
            let raw_vertices = faces
                .iter()
                .map(|&face| {
                    self.faces[face]
                        .indices()
                        .iter()
                        .copied()
                        .find(|&raw| data.topological_vertices[raw as usize] == topological_vertex)
                        .expect("an incident face contains its topology vertex")
                })
                .collect::<BTreeSet<_>>();
            if raw_vertices.len() < faces.len() {
                newly_separated_vertex_count += 1;
            }
        }

        let edges = data
            .edges
            .iter()
            .map(|(&(first, second), incidence)| ([first, second], incidence))
            .collect::<Vec<_>>();
        let mut incident_edges = vec![Vec::new(); data.topological_vertex_count];
        for (edge, (vertices, _)) in edges.iter().enumerate() {
            for &vertex in vertices {
                if affected_vertices[vertex] {
                    incident_edges[vertex].push(edge);
                }
            }
        }
        let face_edges = topology_face_edge_indices(self, &data);
        let mut face_components = vec![Vec::new(); data.topological_vertex_count];
        for topological_vertex in 0..data.topological_vertex_count {
            if !affected_vertices[topological_vertex] {
                continue;
            }
            let faces = &incident_faces[topological_vertex];
            let edge_groups = radially_sorted_vertex_edges(
                topological_vertex,
                &incident_edges[topological_vertex],
                &edges,
                &face_edges,
            );
            // Each face is its own component: no union forest or face-to-local
            // tree is needed. The face walk preserves the same radial order
            // as component ordering with identity parents.
            for (face, _) in radial_vertex_face_walk(&edge_groups, &edges, faces) {
                face_components[topological_vertex].push(vec![face]);
            }
        }

        let vertex_order = (0..data.topological_vertex_count).collect::<Vec<_>>();
        Ok((
            self.rebuilt_from_face_components(&data, &face_components, &vertex_order)?,
            newly_separated_vertex_count,
        ))
    }
}
