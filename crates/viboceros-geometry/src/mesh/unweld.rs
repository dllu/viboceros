//! Angular seam separation through radial face regions.
use super::*;

impl TriangleMesh {
    /// Separates coincident edge endpoints where the incident-face normal
    /// angle is greater than or equal to the supplied tolerance.
    ///
    /// Face regions that remain connected through smoother, already-welded
    /// edges continue to share a raw vertex. Qualifying topology vertices are
    /// rebuilt in OpenNURBS radial order, and unused vertices are compacted as
    /// Rhino does even when no edge qualifies. The returned count is the
    /// number of qualifying topology edges.
    pub fn unwelded_vertices(
        &self,
        angle_tolerance_radians: Real,
    ) -> Result<(Self, usize), GeometryError> {
        self.unwelded_vertices_with_normal_update(angle_tolerance_radians, true)
    }

    /// Separates angular seams while either recomputing stored normal directions
    /// or copying each source normal to its separated raw vertices.
    pub fn unwelded_vertices_with_normal_update(
        &self,
        angle_tolerance_radians: Real,
        modify_normals: bool,
    ) -> Result<(Self, usize), GeometryError> {
        self.unwelded_vertices_precision(angle_tolerance_radians, false, modify_normals)
    }

    /// Uses binary32 face-normal comparisons while retaining binary64 vertices.
    /// Line-network command output uses this normal-channel precision at its
    /// crease boundary; edge splitting shares the ordinary radial machinery.
    pub fn unwelded_vertices_with_float_normals(
        &self,
        angle_tolerance_radians: Real,
    ) -> Result<(Self, usize), GeometryError> {
        self.unwelded_vertices_precision(angle_tolerance_radians, true, true)
    }

    fn unwelded_vertices_precision(
        &self,
        angle_tolerance_radians: Real,
        float_normals: bool,
        modify_normals: bool,
    ) -> Result<(Self, usize), GeometryError> {
        if !(angle_tolerance_radians.is_finite()
            && (0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians))
        {
            return Err(GeometryError::InvalidMeshUnweldAngle);
        }
        let data = self.topology_data();
        let face_normals = self.polygon_face_normals()?;
        let maximum_dot = angle_tolerance_radians.cos();
        let normal_dot = |left: usize, right: usize| -> Result<Real, GeometryError> {
            Ok(if float_normals {
                let a = face_normals[left].as_vector().to_array().map(|v| v as f32);
                let b = face_normals[right].as_vector().to_array().map(|v| v as f32);
                Real::from(a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
            } else {
                face_normals[left]
                    .as_vector()
                    .dot(face_normals[right].as_vector())?
            }
            .clamp(-1., 1.))
        };
        let edges = data
            .edges
            .iter()
            .map(|(&(first, second), incidence)| ([first, second], incidence))
            .collect::<Vec<_>>();
        let mut qualifying_edges = vec![false; edges.len()];
        for (edge_index, (_, incidence)) in edges.iter().enumerate() {
            for (left, right) in incidence.face_pairs() {
                let dot = normal_dot(left, right)?;
                if dot <= maximum_dot {
                    qualifying_edges[edge_index] = true;
                    break;
                }
            }
        }
        let qualifying_edge_count = qualifying_edges.iter().filter(|&&edge| edge).count();
        if qualifying_edge_count == 0 {
            return Ok((self.culled_unused_vertices().0, 0));
        }

        let mut affected_topological_vertices = vec![false; data.topological_vertex_count];
        for (edge, qualifies) in edges.iter().zip(&qualifying_edges) {
            if *qualifies {
                affected_topological_vertices[edge.0[0]] = true;
                affected_topological_vertices[edge.0[1]] = true;
            }
        }
        let face_edges = topology_face_edge_indices(self, &data);
        let mut incident_edges = vec![Vec::new(); data.topological_vertex_count];
        for (edge, (vertices, _)) in edges.iter().enumerate() {
            incident_edges[vertices[0]].push(edge);
            incident_edges[vertices[1]].push(edge);
        }
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
            let mut parents = (0..vertex_faces.len()).collect::<Vec<_>>();
            let mut ranks = vec![0_u8; vertex_faces.len()];
            for &edge in &incident_edges[topological_vertex] {
                let (edge_vertices, incidence) = edges[edge];
                if incidence.count > 2 {
                    continue;
                }
                let endpoint = if edge_vertices[0] == topological_vertex {
                    Some(0)
                } else if edge_vertices[1] == topological_vertex {
                    Some(1)
                } else {
                    None
                };
                let Some(endpoint) = endpoint else {
                    continue;
                };
                for (left, right) in incidence.use_pairs() {
                    let left_local = face_to_local[&left.face];
                    let right_local = face_to_local[&right.face];
                    if left.raw_vertices[endpoint] != right.raw_vertices[endpoint] {
                        continue;
                    }
                    let dot = normal_dot(left.face, right.face)?;
                    if dot > maximum_dot {
                        union_faces(&mut parents, &mut ranks, left_local, right_local);
                    }
                }
            }
            if incident_edges[topological_vertex]
                .iter()
                .any(|&edge| edges[edge].1.count > 2)
            {
                // A non-manifold junction follows the radial face walk, not
                // all pairwise smooth connections across the edge. Qualifying
                // incoming edges split the walk; subsequent adjacent faces can
                // share only when their normals and existing raw indices agree.
                let walk =
                    radial_vertex_face_walk(&edge_groups[topological_vertex], &edges, vertex_faces);
                for pair in walk.windows(2) {
                    let (left, _) = pair[0];
                    let (right, incoming) = pair[1];
                    let Some(incoming) = incoming else {
                        continue;
                    };
                    if qualifying_edges[incoming] {
                        continue;
                    }
                    let raw_at_vertex = |face: usize| {
                        self.faces[face]
                            .indices()
                            .iter()
                            .copied()
                            .find(|&raw| {
                                data.topological_vertices[raw as usize] == topological_vertex
                            })
                            .expect("an incident face contains its topology vertex")
                    };
                    if raw_at_vertex(left) != raw_at_vertex(right) {
                        continue;
                    }
                    let dot = normal_dot(left, right)?;
                    if dot > maximum_dot {
                        union_faces(
                            &mut parents,
                            &mut ranks,
                            face_to_local[&left],
                            face_to_local[&right],
                        );
                    }
                }
            }

            let component_order = ordered_vertex_face_components(
                &edge_groups[topological_vertex],
                &edges,
                &face_to_local,
                vertex_faces,
                &mut parents,
            );
            face_components[topological_vertex] =
                components::faces_in_component_order(vertex_faces, &mut parents, &component_order);
        }
        let vertex_order = (0..data.topological_vertex_count).collect::<Vec<_>>();
        Ok((
            self.rebuilt_from_face_components_with_normal_update(
                &data,
                &face_components,
                &vertex_order,
                modify_normals,
            )?,
            qualifying_edge_count,
        ))
    }
}
