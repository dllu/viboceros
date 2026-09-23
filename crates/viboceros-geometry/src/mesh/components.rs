//! Exact-location mesh connectivity and linear-size component remapping.
use super::*;

#[cfg(test)]
mod tests;

/// Groups local union-find members in the supplied root order while preserving
/// the input face order within each component. Each face is visited once.
pub(super) fn faces_in_component_order(
    incident_faces: &[usize],
    parents: &mut [usize],
    component_order: &[usize],
) -> Vec<Vec<usize>> {
    let mut component_by_root = vec![usize::MAX; parents.len()];
    for (component, &root) in component_order.iter().enumerate() {
        component_by_root[root] = component;
    }
    let mut components = vec![Vec::new(); component_order.len()];
    for (local, &face) in incident_faces.iter().enumerate() {
        let root = index_root(parents, local);
        components
            .get_mut(component_by_root[root])
            .expect("the component order includes every incident face root")
            .push(face);
    }
    components
}

/// Group face indices in first-face order, independently of union root order.
/// Roots are face indices, so a dense lookup avoids tree searches per face.
/// Reject the first over-budget root before allocating its component list.
fn component_faces(
    parents: &mut [usize],
    maximum: usize,
) -> Result<Vec<Vec<usize>>, GeometryError> {
    let mut component_by_root = vec![usize::MAX; parents.len()];
    let mut components = Vec::<Vec<usize>>::new();
    for face in 0..parents.len() {
        let root = index_root(parents, face);
        let component = &mut component_by_root[root];
        if *component == usize::MAX {
            if components.len() == maximum {
                return Err(GeometryError::MeshComponentLimit { maximum });
            }
            *component = components.len();
            components.push(Vec::new());
        }
        components[*component].push(face);
    }
    Ok(components)
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

        Ok(self.pieces_from_faces(component_faces(&mut parents, maximum)?))
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
            if incidence.count == 1 || edge_uses_are_unwelded(incidence.uses()) {
                continue;
            }
            let mut uses = incidence.uses();
            let first = uses.next().expect("a welded edge has incident faces");
            for edge_use in uses {
                union_faces(&mut parents, &mut ranks, first.face, edge_use.face);
            }
        }

        Ok(self.pieces_from_faces(component_faces(&mut parents, maximum)?))
    }

    /// Reuse one raw-vertex map across components. Allocating a complete map
    /// per piece costs O(vertices * pieces) even for isolated triangles.
    fn pieces_from_faces(&self, components: Vec<Vec<usize>>) -> Vec<Self> {
        let mut vertex_remap = vec![None; self.vertices.len()];
        let mut face_remap = if self.ngons.is_empty() {
            Vec::new()
        } else {
            vec![None; self.faces.len()]
        };
        let mut ngon_by_face = if self.ngons.is_empty() {
            Vec::new()
        } else {
            vec![None; self.faces.len()]
        };
        for (ngon_index, ngon) in self.ngons.iter().enumerate() {
            for &face in &ngon.faces {
                ngon_by_face[face as usize] = Some(ngon_index);
            }
        }
        let mut ngon_seen = vec![false; self.ngons.len()];
        components
            .into_iter()
            .map(|faces| {
                let piece = self.piece_from_faces(
                    &faces,
                    &mut vertex_remap,
                    &mut face_remap,
                    &ngon_by_face,
                    &mut ngon_seen,
                );
                // A vertex can belong to multiple edge-disconnected components.
                // Clear exactly the referenced entries, not the entire source map.
                for &face in &faces {
                    if let Some(remapped) = face_remap.get_mut(face) {
                        *remapped = None;
                    }
                    if let Some(ngon) = ngon_by_face.get(face).copied().flatten() {
                        ngon_seen[ngon] = false;
                    }
                    for &vertex in self.faces[face].indices() {
                        vertex_remap[vertex as usize] = None;
                    }
                }
                piece
            })
            .collect()
    }

    fn piece_from_faces(
        &self,
        faces: &[usize],
        vertex_remap: &mut [Option<u32>],
        face_remap: &mut [Option<u32>],
        ngon_by_face: &[Option<usize>],
        ngon_seen: &mut [bool],
    ) -> Self {
        let mut vertices = Vec::new();
        let mut retained_faces = Vec::with_capacity(faces.len());
        let mut ngons = Vec::new();
        for &face in faces {
            if let Some(remapped) = face_remap.get_mut(face) {
                *remapped = Some(
                    u32::try_from(retained_faces.len())
                        .expect("a mesh component cannot have more faces than its source"),
                );
            }
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
            if let Some(ngon) = ngon_by_face.get(face).copied().flatten()
                && !ngon_seen[ngon]
            {
                ngon_seen[ngon] = true;
                ngons.push(ngon);
            }
        }
        ngons.sort_unstable();
        let ngons = ngons
            .into_iter()
            .map(|index| {
                let source = &self.ngons[index];
                MeshNgon::from_parts(
                    source
                        .vertices
                        .iter()
                        .map(|&vertex| {
                            vertex_remap[vertex as usize]
                                .expect("an n-gon boundary vertex belongs to its face component")
                        })
                        .collect(),
                    source
                        .faces
                        .iter()
                        .map(|&face| {
                            face_remap[face as usize]
                                .expect("all n-gon faces belong to one edge component")
                        })
                        .collect(),
                )
            })
            .collect();
        let mut piece = Self::from_validated_parts(vertices, retained_faces);
        piece.ngons = ngons;
        piece
    }
}
