//! Exact-location mesh connectivity and linear-size component remapping.
use super::*;

#[cfg(test)]
mod tests;

/// Group face indices in first-face order, independently of union root order.
/// Roots are face indices, so a dense lookup avoids tree searches per face.
fn component_faces(parents: &mut [usize]) -> Vec<Vec<usize>> {
    let mut component_by_root = vec![usize::MAX; parents.len()];
    let mut components = Vec::<Vec<usize>>::new();
    for face in 0..parents.len() {
        let root = face_root(parents, face);
        let component = &mut component_by_root[root];
        if *component == usize::MAX {
            *component = components.len();
            components.push(Vec::new());
        }
        components[*component].push(face);
    }
    components
}

impl TriangleMesh {
    /// Splits the mesh into exact-location edge-connected components. A lone
    /// shared vertex does not connect faces. Each result retains source face
    /// order and compacts referenced raw vertices in first-use order.
    pub fn disjoint_pieces(&self) -> Vec<Self> {
        self.try_disjoint_pieces(usize::MAX)
            .expect("component count cannot exceed the number of source faces")
    }

    /// Like `disjoint_pieces`, rejecting an excessive component count after
    /// connectivity analysis and before copying component geometry.
    pub fn try_disjoint_pieces(&self, maximum: usize) -> Result<Vec<Self>, GeometryError> {
        let data = self.topology_data();
        let mut parents = (0..self.faces.len()).collect::<Vec<_>>();
        let mut ranks = vec![0_u8; self.faces.len()];
        for incidence in data.edges.values() {
            let mut uses = incidence.uses();
            let Some(first) = uses.next() else {
                continue;
            };
            for edge_use in uses {
                union_faces(&mut parents, &mut ranks, first.face, edge_use.face);
            }
        }

        self.pieces_from_faces_limited(component_faces(&mut parents), maximum)
    }

    /// Splits the mesh into the parts Rhino's `Explode` command sees across
    /// unwelded edges.
    ///
    /// Exact coincident positions establish topological edges. Such an edge
    /// remains welded when its incident faces reuse a raw vertex index at
    /// either endpoint; it is unwelded only when every incident use has a
    /// distinct raw index at both endpoints. A lone shared vertex never joins
    /// parts. Results retain source face order and preserve logical quads.
    pub fn explode_pieces(&self) -> Vec<Self> {
        self.try_explode_pieces(usize::MAX)
            .expect("component count cannot exceed the number of source faces")
    }

    /// Like `explode_pieces`, but rejects an excessive component count after
    /// connectivity analysis and before copying any component geometry.
    pub fn try_explode_pieces(&self, maximum: usize) -> Result<Vec<Self>, GeometryError> {
        let data = self.topology_data();
        let mut parents = (0..self.faces.len()).collect::<Vec<_>>();
        let mut ranks = vec![0_u8; self.faces.len()];
        for incidence in data.edges.values() {
            let uses = incidence.uses().collect::<Vec<_>>();
            if uses.len() == 1 || edge_uses_are_unwelded(&uses) {
                continue;
            }
            for edge_use in &uses[1..] {
                union_faces(&mut parents, &mut ranks, uses[0].face, edge_use.face);
            }
        }

        self.pieces_from_faces_limited(component_faces(&mut parents), maximum)
    }

    fn pieces_from_faces_limited(
        &self,
        components: Vec<Vec<usize>>,
        maximum: usize,
    ) -> Result<Vec<Self>, GeometryError> {
        if components.len() > maximum {
            return Err(GeometryError::MeshComponentLimit { maximum });
        }
        Ok(self.pieces_from_faces(components))
    }

    /// Reuse one raw-vertex map across components. Allocating a complete map
    /// per piece costs O(vertices * pieces) even for isolated triangles.
    fn pieces_from_faces(&self, components: Vec<Vec<usize>>) -> Vec<Self> {
        let mut vertex_remap = vec![None; self.vertices.len()];
        components
            .into_iter()
            .map(|faces| {
                let piece = self.piece_from_faces(&faces, &mut vertex_remap);
                // A vertex can belong to multiple edge-disconnected components.
                // Clear exactly the referenced entries, not the entire source map.
                for &face in &faces {
                    for &vertex in self.faces[face].indices() {
                        vertex_remap[vertex as usize] = None;
                    }
                }
                piece
            })
            .collect()
    }

    fn piece_from_faces(&self, faces: &[usize], vertex_remap: &mut [Option<u32>]) -> Self {
        let mut vertices = Vec::new();
        let mut retained_faces = Vec::with_capacity(faces.len());
        for &face in faces {
            let retained_face = self.faces[face].remapped(|source| {
                let source = source as usize;
                *vertex_remap[source].get_or_insert_with(|| {
                    let target = u32::try_from(vertices.len())
                        .expect("a mesh component cannot have more vertices than its source");
                    vertices.push(self.vertices[source]);
                    target
                })
            });
            retained_faces.push(retained_face);
        }
        Self::from_validated_parts(vertices, retained_faces)
    }
}
