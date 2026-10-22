//! Selected-vertex welding through incident seams.
use super::*;

impl TriangleMesh {
    /// Welds joined mesh seams incident to selected exact-location topology
    /// vertices.
    ///
    /// Indices use the same deterministic order as
    /// [`Self::topology_vertex_points`]. Selecting either endpoint of a seam
    /// welds both endpoint pairs on every incident edge, matching Rhino's
    /// vertex tool. On a non-manifold edge only the first two face uses are
    /// joined. Radial face-corner edits retain established representatives,
    /// and a non-empty valid selection compacts unused vertices. The returned count is the number of incident
    /// edges that required welding.
    pub fn welded_topology_vertices(
        &self,
        vertex_indices: &[usize],
    ) -> Result<(Self, usize), GeometryError> {
        self.welded_topology_vertices_with_normal_generation(vertex_indices, false)
    }

    /// Generates a missing source normal channel when requested, then blends
    /// stored UVs, normals and colors across the merged endpoint pairs.
    pub fn welded_topology_vertices_with_normal_generation(
        &self,
        vertex_indices: &[usize],
        generate_normals: bool,
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
        let edges = data
            .edges
            .iter()
            .map(|(&(a, b), uses)| ([a, b], uses))
            .collect::<Vec<_>>();
        let mut incident = vec![Vec::new(); vertex_count];
        for (edge, (vertices, _)) in edges.iter().enumerate() {
            for &vertex in vertices {
                incident[vertex].push(edge);
            }
        }
        let face_edges = topology_face_edge_indices(self, &data);
        let normals = if generate_normals {
            self.stored_or_computed_normals()?
        } else {
            self.vertex_normals().map(std::borrow::Cow::Borrowed)
        };
        let mut attributes = weld::attributes::WeldAttributes::new(self, normals.as_deref());
        let mut corners = self
            .faces
            .iter()
            .map(|face| {
                let mut corners = [0; 4];
                corners[..face.indices().len()].copy_from_slice(face.indices());
                corners
            })
            .collect::<Vec<_>>();
        let mut formed = vec![false; self.vertices.len()];
        let mut counted = vec![false; edges.len()];
        let mut welded_edge_count = 0;
        let order = selected_vertex_component_order(&selected_vertices, &incident, &edges);
        for vertex in order {
            let groups =
                radially_sorted_vertex_edges(vertex, &incident[vertex], &edges, &face_edges);
            for edge in groups.into_iter().flatten() {
                let (vertices, incidence) = edges[edge];
                let mut uses = incidence.uses();
                let Some(first) = uses.next() else {
                    continue;
                };
                let Some(second) = uses.next() else {
                    continue;
                };
                let uses = [first, second];
                let positions = uses.map(|u| {
                    let next = (u.side + 1) % self.faces[u.face].indices().len();
                    if u.forward {
                        [u.side, next]
                    } else {
                        [next, u.side]
                    }
                });
                let current: [[u32; 2]; 2] = std::array::from_fn(|side| {
                    positions[side].map(|corner| corners[uses[side].face][corner])
                });
                let endpoint = usize::from(vertices[1] == vertex);
                let a = current[0][endpoint] as usize;
                let b = current[1][endpoint] as usize;
                let oriented = first.forward != second.forward;
                let closing = a == b
                    && first.raw_vertices[endpoint] != second.raw_vertices[endpoint]
                    && oriented;
                // A closed fan's final join carries the established face's
                // attributes; the other incident endpoint is copied, not
                // averaged again. Originally shared endpoints still blend
                // the other endpoint normally.
                let side = if closing {
                    usize::from(
                        b == second.raw_vertices[endpoint] as usize
                            && a != first.raw_vertices[endpoint] as usize,
                    )
                } else if formed[a] != formed[b] {
                    usize::from(formed[b])
                } else if oriented {
                    // Start with the incoming face at the selected vertex.
                    usize::from(first.forward != (endpoint == 1))
                } else {
                    usize::from(a < b)
                };
                for end in 0..2 {
                    let a = current[0][end] as usize;
                    let b = current[1][end] as usize;
                    let keep = current[side][end];
                    if a != b {
                        if !closing {
                            attributes.blend(a, b, true)?;
                        }
                        formed[keep as usize] = true;
                    }
                    // Rewrite this pair of face corners, rather than all
                    // aliases of a raw vertex: overlapping selected fans can
                    // retain separate sharing at neighboring vertices.
                    for side in 0..2 {
                        corners[uses[side].face][positions[side][end]] = keep;
                    }
                }
                if first.raw_vertices != second.raw_vertices
                    && !std::mem::replace(&mut counted[edge], true)
                {
                    welded_edge_count += 1;
                }
            }
        }
        let faces = self
            .faces
            .iter()
            .zip(corners)
            .map(|(face, raw)| match face {
                MeshFace::Triangle(_) => MeshFace::Triangle([raw[0], raw[1], raw[2]]),
                MeshFace::Quad(_) => MeshFace::Quad(raw),
            })
            .collect::<Vec<_>>();
        let (welded, _) = self.compacted_with_face_channels(
            &faces,
            &attributes.channels,
            attributes.colors.as_deref(),
        );
        Ok((welded, welded_edge_count))
    }
}

// Adjacent selections start at the incoming endpoint of their first edge.
// Discover components once, so separate selections do not rescan the mesh.
fn selected_vertex_component_order(
    selected: &[bool],
    incident: &[Vec<usize>],
    edges: &[([usize; 2], &EdgeIncidence)],
) -> Vec<usize> {
    let mut seen = vec![false; selected.len()];
    let mut ordered = vec![false; selected.len()];
    let mut result = Vec::new();
    for original in 0..selected.len() {
        if !selected[original] || seen[original] {
            continue;
        }
        let mut queue = VecDeque::from([original]);
        seen[original] = true;
        let mut first_edge: Option<usize> = None;
        while let Some(vertex) = queue.pop_front() {
            for &edge in &incident[vertex] {
                let (vertices, _) = edges[edge];
                let other = vertices[usize::from(vertices[0] == vertex)];
                if selected[other] {
                    first_edge = Some(first_edge.map_or(edge, |previous| previous.min(edge)));
                    if !std::mem::replace(&mut seen[other], true) {
                        queue.push_back(other);
                    }
                }
            }
        }
        let start = first_edge.map_or(original, |edge| {
            let (vertices, incidence) = edges[edge];
            vertices[usize::from(incidence.uses().next().unwrap().forward)]
        });
        queue.push_back(start);
        ordered[start] = true;
        while let Some(vertex) = queue.pop_front() {
            result.push(vertex);
            for &edge in &incident[vertex] {
                let (vertices, _) = edges[edge];
                let other = vertices[usize::from(vertices[0] == vertex)];
                if selected[other] && !std::mem::replace(&mut ordered[other], true) {
                    queue.push_back(other);
                }
            }
        }
    }
    result
}
