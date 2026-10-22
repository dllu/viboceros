//! Selected-edge separation through local radial face components.
use super::rebuild::FaceComponentRebuildPolicy;
use super::*;

impl TriangleMesh {
    /// Separates the supplied exact-location topology edges.
    ///
    /// Indices use the same deterministic order as [`Self::wireframe_lines`].
    /// Naked and already-unwelded edges require no new seams, but a non-empty
    /// valid selection still compacts unused vertices as Rhino does. The
    /// returned count is the number of distinct edges that required a new
    /// separation. Each endpoint is separated only if all incident edge faces
    /// share one raw vertex there; existing partial sharing is preserved.
    pub fn unwelded_topology_edges(
        &self,
        edge_indices: &[usize],
    ) -> Result<(Self, usize), GeometryError> {
        self.unwelded_topology_edges_with_normal_update(edge_indices, true)
    }

    /// Separates selected topology edges while optionally updating stored
    /// normals for replacement vertices. Retained vertices keep their normals.
    pub fn unwelded_topology_edges_with_normal_update(
        &self,
        edge_indices: &[usize],
        modify_normals: bool,
    ) -> Result<(Self, usize), GeometryError> {
        if edge_indices.is_empty() {
            return Ok((self.clone(), 0));
        }
        let data = self.topology_data();
        let edge_count = data.edges.len();
        if let Some(&edge) = edge_indices.iter().find(|&&edge| edge >= edge_count) {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange { edge, edge_count });
        }
        let edges = data
            .edges
            .iter()
            .map(|(&(first, second), incidence)| ([first, second], incidence))
            .collect::<Vec<_>>();
        let mut active_edges = vec![false; edge_count];
        for &edge in edge_indices {
            active_edges[edge] = true;
        }
        for ((_, incidence), active) in edges.iter().zip(&mut active_edges) {
            *active = *active
                && (edge_endpoint_is_fully_shared(incidence, 0)
                    || edge_endpoint_is_fully_shared(incidence, 1));
        }
        let active_edge_count = active_edges.iter().filter(|&&active| active).count();
        if active_edge_count == 0 {
            return Ok((self.culled_unused_vertices().0, 0));
        }

        let mut affected_topological_vertices = vec![false; data.topological_vertex_count];
        let mut incident_edges = vec![Vec::new(); data.topological_vertex_count];
        for (edge, (vertices, _)) in edges.iter().enumerate() {
            incident_edges[vertices[0]].push(edge);
            incident_edges[vertices[1]].push(edge);
            if active_edges[edge] {
                for endpoint in 0..2 {
                    affected_topological_vertices[vertices[endpoint]] |=
                        edge_endpoint_is_fully_shared(edges[edge].1, endpoint);
                }
            }
        }
        let face_edges = topology_face_edge_indices(self, &data);
        let mut edge_groups = vec![Vec::new(); data.topological_vertex_count];
        for vertex in 0..data.topological_vertex_count {
            if affected_topological_vertices[vertex] {
                edge_groups[vertex] = radially_sorted_vertex_edges(
                    vertex,
                    &incident_edges[vertex],
                    &edges,
                    &face_edges,
                );
            }
        }
        let mut incident_faces = vec![Vec::new(); data.topological_vertex_count];
        for (face, polygon) in self.faces.iter().enumerate() {
            for &raw in polygon.indices() {
                let incident = &mut incident_faces[data.topological_vertices[raw as usize]];
                if incident.last().copied() != Some(face) {
                    incident.push(face);
                }
            }
        }

        let mut face_components = vec![Vec::new(); data.topological_vertex_count];
        let mut policy = FaceComponentRebuildPolicy {
            modify_normals,
            ..Default::default()
        };
        let mut normal_cache = BTreeMap::<usize, [Real; 3]>::new();
        for topological_vertex in 0..data.topological_vertex_count {
            if !affected_topological_vertices[topological_vertex] {
                continue;
            }
            let vertex_faces = &incident_faces[topological_vertex];
            let face_to_local = vertex_faces
                .iter()
                .enumerate()
                .map(|(local, &face)| (face, local))
                .collect::<BTreeMap<_, _>>();
            let vertex_edges = &incident_edges[topological_vertex];
            // Incident edges were appended in global index order. Keep only
            // this vertex's flags, avoiding a whole-mesh mask clone per vertex.
            let local_edge = |edge| {
                vertex_edges
                    .binary_search(&edge)
                    .expect("a radial edge is incident to its topology vertex")
            };
            // Rhino only separates an endpoint when every face along this
            // edge uses the same raw vertex there. Existing partial sharing
            // at a non-manifold endpoint is preserved, not fully separated.
            let mut separated_edges = vertex_edges
                .iter()
                .map(|&edge| {
                    let (vertices, incidence) = edges[edge];
                    let endpoint = usize::from(vertices[1] == topological_vertex);
                    active_edges[edge] && edge_endpoint_is_fully_shared(incidence, endpoint)
                })
                .collect::<Vec<_>>();
            let mut artificial_fans = Vec::new();
            for (group_index, group) in edge_groups[topological_vertex].iter().enumerate() {
                let selected = group
                    .iter()
                    .enumerate()
                    .filter_map(|(position, &edge)| {
                        separated_edges[local_edge(edge)].then_some(position)
                    })
                    .collect::<Vec<_>>();
                let closed_manifold = group.iter().all(|&edge| edges[edge].1.count == 2);
                if closed_manifold && selected.len() == 1 {
                    separated_edges[local_edge(group[(selected[0] + 1) % group.len()])] = true;
                    artificial_fans.push((group_index, group[selected[0]]));
                }
            }

            let mut parents = (0..vertex_faces.len()).collect::<Vec<_>>();
            let mut ranks = vec![0_u8; vertex_faces.len()];
            for (&edge, &separated) in vertex_edges.iter().zip(&separated_edges) {
                let (edge_vertices, incidence) = edges[edge];
                let endpoint = usize::from(edge_vertices[1] == topological_vertex);
                debug_assert_eq!(edge_vertices[endpoint], topological_vertex);
                if separated {
                    // The native edge edit separates its first two incident
                    // faces; additional non-manifold uses keep their sharing.
                    let mut remaining = incidence.uses().skip(2);
                    if let Some(first) = remaining.next() {
                        for next in remaining {
                            if first.raw_vertices[endpoint] == next.raw_vertices[endpoint] {
                                union_faces(
                                    &mut parents,
                                    &mut ranks,
                                    face_to_local[&first.face],
                                    face_to_local[&next.face],
                                );
                            }
                        }
                    }
                    continue;
                }
                for (left, right) in incidence.use_pairs() {
                    if left.raw_vertices[endpoint] == right.raw_vertices[endpoint] {
                        union_faces(
                            &mut parents,
                            &mut ranks,
                            face_to_local[&left.face],
                            face_to_local[&right.face],
                        );
                    }
                }
            }

            let mut component_order = ordered_vertex_face_components(
                &edge_groups[topological_vertex],
                &edges,
                &face_to_local,
                vertex_faces,
                &mut parents,
            );
            component_order.reverse();
            // Each selected crease advances the radial start. The final
            // selected edge supplies its outgoing face as the first component.
            let first_face = vertex_edges.iter().rev().find_map(|&edge| {
                if !active_edges[edge] {
                    return None;
                }
                let (vertices, incidence) = edges[edge];
                incidence
                    .uses()
                    .find(|u| u.forward == (vertices[0] == topological_vertex))
                    .map(|u| u.face)
            });
            if let Some(face) = first_face {
                let root = index_root(&mut parents, face_to_local[&face]);
                if let Some(start) = component_order.iter().position(|&r| r == root) {
                    component_order.rotate_left(start);
                }
            }
            face_components[topological_vertex] =
                components::faces_in_component_order(vertex_faces, &mut parents, &component_order);
            let edited_faces = vertex_edges
                .iter()
                .filter(|&&edge| {
                    let (vertices, incidence) = edges[edge];
                    active_edges[edge]
                        && edge_endpoint_is_fully_shared(
                            incidence,
                            usize::from(vertices[1] == topological_vertex),
                        )
                })
                .flat_map(|&edge| edges[edge].1.uses().take(2).map(|u| u.face))
                .collect::<BTreeSet<_>>();
            for (component_index, component) in
                face_components[topological_vertex].iter().enumerate()
            {
                let raw_at_vertex = |face: usize| {
                    self.faces[face]
                        .indices()
                        .iter()
                        .copied()
                        .find(|&raw| data.topological_vertices[raw as usize] == topological_vertex)
                        .unwrap()
                };
                let source = raw_at_vertex(component[0]);
                if component.iter().all(|f| !edited_faces.contains(f)) {
                    policy
                        .retained
                        .insert((topological_vertex, component_index), source);
                    continue;
                }
                if !modify_normals {
                    continue;
                }
                for &(group_index, edge) in &artificial_fans {
                    let pair = edges[edge]
                        .1
                        .uses()
                        .take(2)
                        .map(|u| u.face)
                        .collect::<Vec<_>>();
                    let opposite =
                        match (component.contains(&pair[0]), component.contains(&pair[1])) {
                            (true, false) => pair[1],
                            (false, true) => pair[0],
                            _ => continue,
                        };
                    // A forced extra cut makes the topology representable,
                    // but its neighboring faces still contribute to shading
                    // on both sides of the explicitly selected crease.
                    let faces = edge_groups[topological_vertex][group_index]
                        .iter()
                        .flat_map(|&e| edges[e].1.uses().map(|u| u.face))
                        .collect::<BTreeSet<_>>();
                    let mut sum = [0.; 3];
                    for face in faces {
                        if face == opposite || raw_at_vertex(face) != source {
                            continue;
                        }
                        let normal = if let Some(&normal) = normal_cache.get(&face) {
                            normal
                        } else {
                            let normal = self
                                .polygon_face_normal(face)?
                                .as_vector()
                                .to_array()
                                .map(|n| n as f32 as Real);
                            normal_cache.insert(face, normal);
                            normal
                        };
                        for (s, n) in sum.iter_mut().zip(normal) {
                            *s += n;
                        }
                    }
                    let normal = Vector3::try_from(sum)?;
                    let normal = normal
                        .normalized_nonzero()
                        .map(UnitVector3::as_vector)
                        .unwrap_or(normal);
                    let normal = Vector3::try_from(normal.to_array().map(|n| n as f32 as Real))?;
                    policy
                        .normal_overrides
                        .insert((topological_vertex, component_index), normal);
                }
            }
        }

        let mut vertex_order = Vec::new();
        let mut seen = vec![false; data.topological_vertex_count];
        for face in &self.faces {
            for &raw in face.indices() {
                let vertex = data.topological_vertices[raw as usize];
                if affected_topological_vertices[vertex]
                    && !std::mem::replace(&mut seen[vertex], true)
                {
                    vertex_order.push(vertex);
                }
            }
        }
        let normals = self.stored_or_computed_normals()?;
        policy.normals = normals.as_deref();
        Ok((
            self.rebuilt_from_component_plan(&data, &face_components, &vertex_order, policy)?,
            active_edge_count,
        ))
    }
}
