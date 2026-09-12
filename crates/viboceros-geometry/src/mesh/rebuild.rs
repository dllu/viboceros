//! Shared face-component rebuilding for angle, edge, and vertex unwelding.
use super::*;

#[cfg(test)]
mod tests;

fn output_vertex_count(
    retained: usize,
    component_counts: impl IntoIterator<Item = usize>,
) -> Result<usize, GeometryError> {
    let count = component_counts
        .into_iter()
        .try_fold(retained, |total, added| {
            total
                .checked_add(added)
                .ok_or(GeometryError::TooManyMeshVertices)
        })?;
    if count
        .checked_sub(1)
        .is_some_and(|last| u32::try_from(last).is_err())
    {
        return Err(GeometryError::TooManyMeshVertices);
    }
    Ok(count)
}

impl TriangleMesh {
    pub(super) fn rebuilt_from_face_components(
        &self,
        data: &MeshTopologyData,
        face_components: &[Vec<Vec<usize>>],
        topological_vertex_order: &[usize],
    ) -> Result<Self, GeometryError> {
        let affected_topological_vertices = face_components
            .iter()
            .map(|components| !components.is_empty())
            .collect::<Vec<_>>();
        let mut used = vec![false; self.vertices.len()];
        for face in &self.faces {
            for &vertex in face.indices() {
                used[vertex as usize] =
                    !affected_topological_vertices[data.topological_vertices[vertex as usize]];
            }
        }
        let retained = used.iter().filter(|&&used| used).count();
        let vertex_count = output_vertex_count(
            retained,
            topological_vertex_order
                .iter()
                .map(|&vertex| face_components[vertex].len()),
        )?;
        let mut raw_remap = Vec::new();
        raw_remap
            .try_reserve_exact(self.vertices.len())
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        raw_remap.resize(self.vertices.len(), 0_u32);
        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        for (source, (&point, is_used)) in self.vertices.iter().zip(used).enumerate() {
            if !is_used {
                continue;
            }
            raw_remap[source] =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            vertices.push(point);
        }

        // Faces have at most four corners. Corner-indexed slots avoid a tree
        // allocation per face while keeping missing replacements explicit;
        // every u32 value, including MAX, remains a valid replacement index.
        let mut face_replacements = Vec::new();
        face_replacements
            .try_reserve_exact(self.faces.len())
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        face_replacements.resize(self.faces.len(), [None; 4]);
        for (face_index, face) in self.faces.iter().enumerate() {
            for (corner, &raw_vertex) in face.indices().iter().enumerate() {
                let source = raw_vertex as usize;
                if !affected_topological_vertices[data.topological_vertices[source]] {
                    face_replacements[face_index][corner] = Some(raw_remap[source]);
                }
            }
        }
        for &topological_vertex in topological_vertex_order {
            let components = &face_components[topological_vertex];
            for component in components {
                let target = u32::try_from(vertices.len())
                    .map_err(|_| GeometryError::TooManyMeshVertices)?;
                vertices.push(data.topological_points[topological_vertex]);
                for &face in component {
                    let corner = self.faces[face]
                        .indices()
                        .iter()
                        .position(|&raw| {
                            data.topological_vertices[raw as usize] == topological_vertex
                        })
                        .expect("an incident face contains its topology vertex");
                    face_replacements[face][corner] = Some(target);
                }
            }
        }

        debug_assert_eq!(vertices.len(), vertex_count);
        let mut faces = Vec::new();
        faces
            .try_reserve_exact(self.faces.len())
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        faces.extend(
            self.faces
                .iter()
                .copied()
                .enumerate()
                .map(|(face, polygon)| {
                    let mut corners = face_replacements[face].into_iter();
                    polygon.remapped(|_| {
                        corners
                            .next()
                            .expect("a mesh face has at most four corners")
                            .expect("every unwelded face vertex has a replacement")
                    })
                }),
        );
        Ok(Self::from_validated_parts(vertices, faces))
    }
}
