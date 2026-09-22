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
        let mut parents = (0..self.faces.len()).collect::<Vec<_>>();
        let mut ranks = vec![0_u8; self.faces.len()];
        let mut first = vec![usize::MAX; self.edges.len()];
        for (face, value) in self.faces.iter().enumerate() {
            for trim in value.loops.iter().flat_map(|boundary| &boundary.trims) {
                let Some(edge) = trim.edge else { continue };
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
