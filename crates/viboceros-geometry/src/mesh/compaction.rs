//! Source-ordered compaction with explicit raw-vertex attribute remapping.
use super::*;

impl TriangleMesh {
    /// Compacts coincident-vertex unions in source order, consuming the parent
    /// forest as scratch storage so it cannot be reused after remapping.
    pub(super) fn compacted_with_vertex_parents(&self, parents: Vec<usize>) -> (Self, usize) {
        self.compacted_with_vertex_parents_and_channels(
            parents,
            &self.vertex_channels,
            self.vertex_colors.as_deref(),
        )
    }

    pub(super) fn compacted_with_vertex_parents_and_channels(
        &self,
        parents: Vec<usize>,
        input_channels: &VertexChannels,
        input_colors: Option<&[[u8; 4]]>,
    ) -> (Self, usize) {
        self.compacted_with_vertex_channel_builder(
            parents,
            ChannelBuilder::new(input_channels),
            input_colors,
            self.vertex_normals().is_some() || input_channels.normals.is_none(),
            &self.faces,
        )
    }

    pub(super) fn compacted_with_vertex_parents_and_normals(
        &self,
        parents: Vec<usize>,
        normals: Option<&[Vector3]>,
    ) -> (Self, usize) {
        self.compacted_with_vertex_channel_builder(
            parents,
            ChannelBuilder::with_normals(&self.vertex_channels, normals),
            self.vertex_colors.as_deref(),
            self.vertex_normals().is_some() || normals.is_none(),
            &self.faces,
        )
    }

    pub(super) fn compacted_with_face_channels(
        &self,
        faces: &[MeshFace],
        channels: &VertexChannels,
        colors: Option<&[[u8; 4]]>,
    ) -> (Self, usize) {
        self.compacted_with_vertex_channel_builder(
            (0..self.vertices.len()).collect(),
            ChannelBuilder::new(channels),
            colors,
            false,
            faces,
        )
    }

    fn compacted_with_vertex_channel_builder(
        &self,
        mut parents: Vec<usize>,
        mut channels: ChannelBuilder<'_>,
        input_colors: Option<&[[u8; 4]]>,
        keep_original_if_uncompacted: bool,
        input_faces: &[MeshFace],
    ) -> (Self, usize) {
        let mut retained = vec![false; self.vertices.len()];
        let mut retained_count = 0;
        for face in input_faces {
            for &vertex in face.indices() {
                let representative = index_root(&mut parents, vertex as usize);
                if !retained[representative] {
                    retained[representative] = true;
                    retained_count += 1;
                }
            }
        }
        let removed = self.vertices.len() - retained_count;
        if removed == 0 && keep_original_if_uncompacted {
            return (self.clone(), 0);
        }

        // Resolve all face references before reusing the parent forest as a
        // representative-to-compact-index table. Rewriting a root earlier
        // would corrupt later root searches (including unused source roots).
        let mut faces = input_faces
            .iter()
            .copied()
            .map(|face| {
                face.remapped(|vertex| {
                    u32::try_from(index_root(&mut parents, vertex as usize))
                        .expect("a representative is an existing source vertex")
                })
            })
            .collect::<Vec<_>>();
        let mut vertices = Vec::with_capacity(retained_count);
        let mut colors = input_colors.map(|_| Vec::with_capacity(retained_count));
        for (source, (&point, keep)) in self.vertices.iter().zip(retained).enumerate() {
            if !keep {
                continue;
            }
            parents[source] = vertices.len();
            vertices.push(point);
            channels.push(source);
            if let Some(colors) = &mut colors {
                colors.push(input_colors.unwrap()[source]);
            }
        }
        for face in &mut faces {
            *face = face.remapped(|representative| {
                u32::try_from(parents[representative as usize])
                    .expect("a compacted mesh cannot have more vertices than its source")
            });
        }
        let mut compacted = Self::from_validated_parts(vertices, faces)
            .retain_valid_ngons_for_same_faces(&self.ngons);
        compacted.vertex_colors = colors;
        compacted.vertex_channels = channels.finish();
        (compacted, removed)
    }

    /// Removes vertices that are not referenced by any face. Referenced
    /// vertices retain their relative source order, and coincident referenced
    /// vertices remain distinct. Face order and winding are unchanged.
    pub fn culled_unused_vertices(&self) -> (Self, usize) {
        let mut used = vec![false; self.vertices.len()];
        for face in &self.faces {
            for &vertex in face.indices() {
                used[vertex as usize] = true;
            }
        }

        let retained_vertex_count = used.iter().filter(|&&is_used| is_used).count();
        let removed_vertex_count = self.vertices.len() - retained_vertex_count;
        if removed_vertex_count == 0 {
            return (self.clone(), 0);
        }

        let mut vertex_remap = vec![0_u32; self.vertices.len()];
        let mut channels = ChannelBuilder::new(&self.vertex_channels);
        let mut vertices = Vec::with_capacity(retained_vertex_count);
        let mut colors = self
            .vertex_colors
            .as_ref()
            .map(|_| Vec::with_capacity(retained_vertex_count));
        for (source, (&point, is_used)) in self.vertices.iter().zip(used).enumerate() {
            if !is_used {
                continue;
            }
            vertex_remap[source] = u32::try_from(vertices.len())
                .expect("a culled mesh cannot have more vertices than its source");
            vertices.push(point);
            channels.push(source);
            if let Some(colors) = &mut colors {
                colors.push(self.vertex_colors.as_ref().unwrap()[source]);
            }
        }
        let faces = self
            .faces
            .iter()
            .copied()
            .map(|face| face.remapped(|vertex| vertex_remap[vertex as usize]))
            .collect();
        let mut culled = Self::from_validated_parts(vertices, faces);
        culled.vertex_colors = colors;
        culled.vertex_channels = channels.finish();
        culled.ngons = self
            .ngons
            .iter()
            .map(|ngon| MeshNgon {
                vertices: ngon
                    .vertices
                    .iter()
                    .map(|&vertex| vertex_remap[vertex as usize])
                    .collect(),
                faces: ngon.faces.clone(),
            })
            .collect();
        (culled, removed_vertex_count)
    }
}
