//! Face connectivity through shared topological edges, independent of embedding.
use super::Brep;

#[cfg(test)]
mod tests;

impl Brep {
    /// Partitions face indices by shared topological edges.
    ///
    /// Each face occurs exactly once. Components are ordered by their smallest
    /// face index, and each component's indices are ascending. Seam edges,
    /// nonmanifold edges, and isolated faces are supported. Singular trims have
    /// no edge and do not connect faces. Neither vertex-only contact nor spatial
    /// coincidence joins otherwise independent shells.
    ///
    /// This query does not rebuild geometry, change face sense, classify spatial
    /// containment, or establish that a component bounds a valid solid. Storage
    /// is linear in faces and edges; union/find work is near-linear in trim uses.
    pub fn edge_connected_face_components(&self) -> Vec<Vec<usize>> {
        self.edge_connected_face_components_where(|_| true)
    }

    /// Splits the complete B-rep by shared edges, preserving face/vertex/edge
    /// order, geometry, tolerances and orientation in each component. Contacts
    /// at a vertex alone stay separate. Global tables are scanned once; local
    /// references are sorted and compacted without rescanning every component.
    pub fn into_edge_connected_components(
        self,
        tolerance: super::Tolerance,
    ) -> Result<Vec<Self>, super::GeometryError> {
        let components = self.edge_connected_face_components();
        if components.len() == 1 {
            return Ok(vec![self]);
        }
        self.copy_face_components(&components, tolerance)
    }

    // The caller supplies the complete edge-connected partition. Keeping this
    // internal avoids silently retaining mated trim flags for arbitrary subsets.
    pub(crate) fn copy_face_components(
        &self,
        components: &[Vec<usize>],
        tolerance: super::Tolerance,
    ) -> Result<Vec<Self>, super::GeometryError> {
        let mut outputs = Vec::with_capacity(components.len());
        let mut vertex_map = vec![0; self.vertices.len()];
        let mut edge_map = vec![0; self.edges.len()];
        for faces in components {
            let mut vertex_ids = Vec::new();
            let mut edge_ids = Vec::new();
            for &f in faces {
                for trim in self.faces[f].loops.iter().flat_map(|l| &l.trims) {
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
                    self.vertices[v]
                })
                .collect();
            let edges = edge_ids
                .into_iter()
                .enumerate()
                .map(|(i, e)| {
                    edge_map[e] = i;
                    let mut edge = self.edges[e].clone();
                    edge.vertices = edge.vertices.map(|v| vertex_map[v]);
                    edge
                })
                .collect();
            let faces = faces
                .iter()
                .map(|&f| {
                    let mut face = self.faces[f].clone();
                    for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
                        trim.vertices = trim.vertices.map(|v| vertex_map[v]);
                        trim.edge = trim.edge.map(|e| edge_map[e]);
                    }
                    face
                })
                .collect();
            outputs.push(Self::try_new(vertices, edges, faces, tolerance)?);
        }
        Ok(outputs)
    }

    pub(super) fn edge_connected_face_components_where(
        &self,
        include: impl Fn(usize) -> bool,
    ) -> Vec<Vec<usize>> {
        let mut parents = (0..self.faces.len()).collect::<Vec<_>>();
        let mut ranks = vec![0_u8; self.faces.len()];
        let mut first = vec![usize::MAX; self.edges.len()];
        for (face, value) in self.faces.iter().enumerate() {
            for trim in value.loops.iter().flat_map(|boundary| &boundary.trims) {
                let Some(edge) = trim.edge else { continue };
                if !include(edge) {
                    continue;
                }
                if first[edge] == usize::MAX {
                    first[edge] = face;
                } else {
                    let mut a = root(&mut parents, face);
                    let mut b = root(&mut parents, first[edge]);
                    if a != b {
                        if ranks[a] < ranks[b] {
                            std::mem::swap(&mut a, &mut b);
                        }
                        parents[b] = a;
                        if ranks[a] == ranks[b] {
                            ranks[a] += 1;
                        }
                    }
                }
            }
        }
        let mut root_component = vec![usize::MAX; self.faces.len()];
        let mut components: Vec<Vec<usize>> = Vec::new();
        for face in 0..self.faces.len() {
            let representative = root(&mut parents, face);
            if root_component[representative] == usize::MAX {
                root_component[representative] = components.len();
                components.push(Vec::new());
            }
            components[root_component[representative]].push(face);
        }
        components
    }
}

fn root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        parents[index] = parents[parents[index]];
        index = parents[index];
    }
    index
}
