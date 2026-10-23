//! Face orientation constraints and explicit winding repair.
use super::*;

impl TriangleMesh {
    /// SDK-style winding unification. Stored vertex normals referenced
    /// by flipped faces are negated once; unrelated channels and n-gon cycles
    /// remain unchanged. Nonmanifold edges follow their first incident face.
    /// This operation does not guarantee outward orientation.
    pub fn unified_normals(&self) -> Result<(Self, usize), GeometryError> {
        let flipped = self.normal_unification_plan();
        let count = flipped.iter().filter(|&&f| f).count();
        if count == 0 {
            return Ok((self.clone(), 0));
        }
        let faces = self
            .faces
            .iter()
            .zip(&flipped)
            .map(|(&face, &flip)| if flip { face.reversed() } else { face })
            .collect();
        let mut result = Self::from_validated_parts(self.vertices.clone(), faces)
            .try_with_ngons(self.ngons.clone())?;
        result.vertex_colors = self.vertex_colors.clone();
        result.vertex_channels = self.vertex_channels.clone();
        if let Some(normals) = result.vertex_channels.normals.as_mut() {
            let mut affected = vec![false; self.vertices.len()];
            for (face, flip) in self.faces.iter().zip(flipped) {
                if flip {
                    for &v in face.indices() {
                        affected[v as usize] = true;
                    }
                }
            }
            for (normal, flip) in normals.iter_mut().zip(affected) {
                if flip {
                    *normal = Vector3::try_from(normal.to_array().map(|x| -x))?;
                }
            }
        }
        Ok((result, count))
    }

    /// Count the SDK face edits without copying geometry or stored channels.
    pub fn normal_unification_count(&self) -> usize {
        self.normal_unification_plan()
            .into_iter()
            .filter(|f| *f)
            .count()
    }

    /// Command policy: rebuild normals and orient n-gon boundaries on changed
    /// meshes, preserving unused vertices and all other stored channels.
    pub fn unified_normals_for_command(&self) -> Result<(Self, usize), GeometryError> {
        let (mesh, count) = self.unified_normals()?;
        if count == 0 {
            return Ok((mesh, count));
        }
        let data =
            viboceros_opennurbs::recompute_mesh_normals_and_orient_ngons(&mesh.open_nurbs_data())?;
        Ok((Self::from_open_nurbs_data(data)?, count))
    }

    fn normal_unification_plan(&self) -> Vec<bool> {
        let data = self.topology_data();
        let mut neighbors = vec![Vec::<(usize, bool)>::new(); self.faces.len()];
        for incidence in data.edges.values() {
            let Some(first) = incidence.first_use else {
                continue;
            };
            for other in incidence.uses().skip(1) {
                let opposite = first.forward == other.forward;
                neighbors[first.face].push((other.face, opposite));
                neighbors[other.face].push((first.face, opposite));
            }
        }
        let reversed_closing_edge = |face: usize| {
            let indices = self.faces[face].indices();
            data.topological_vertices[indices[0] as usize]
                < data.topological_vertices[*indices.last().unwrap() as usize]
        };
        // Seed the first component from a closing side directed toward a lower
        // topology vertex index, then visit unassigned components from the end.
        // Cyclic origin and topology numbering both affect the initial seed.
        let roots = (0..self.faces.len())
            .find(|&f| reversed_closing_edge(f))
            .into_iter()
            .chain((0..self.faces.len()).rev());
        let mut assigned = vec![None; self.faces.len()];
        let mut pending = VecDeque::new();
        for root in roots {
            if assigned[root].is_some() {
                continue;
            }
            assigned[root] = Some(false);
            pending.push_back(root);
            while let Some(face) = pending.pop_front() {
                let orientation = assigned[face].unwrap();
                for &(next, opposite) in &neighbors[face] {
                    if assigned[next].is_none() {
                        assigned[next] = Some(orientation ^ opposite);
                        pending.push_back(next);
                    }
                }
            }
        }
        assigned.into_iter().map(|f| f.unwrap()).collect()
    }

    /// Reorients each manifold-connected face component consistently while
    /// retaining face and vertex order. Exact coincident locations define
    /// adjacency, as they do for [`Self::topology`]. Non-manifold edges do not
    /// impose an ambiguous orientation constraint.
    pub fn unified_face_orientations(&self) -> Result<(Self, usize), GeometryError> {
        let data = self.topology_data();
        let mut neighbors = vec![Vec::<(usize, bool)>::new(); self.faces.len()];
        for incidence in data.edges.values() {
            if incidence.count != 2 {
                continue;
            }
            let first = incidence
                .first_use
                .expect("an edge used twice records its first face");
            let second = incidence
                .second_use
                .expect("an edge used twice records its second face");
            // Equal traversal directions require exactly one adjacent face to
            // flip; opposite directions require both to retain equal parity.
            let opposite_parity = first.forward == second.forward;
            neighbors[first.face].push((second.face, opposite_parity));
            neighbors[second.face].push((first.face, opposite_parity));
        }

        let mut flipped = vec![None; self.faces.len()];
        let mut pending = VecDeque::new();
        for root in 0..self.faces.len() {
            if flipped[root].is_some() {
                continue;
            }
            flipped[root] = Some(false);
            pending.push_back(root);
            while let Some(face) = pending.pop_front() {
                let face_flipped = flipped[face].expect("queued faces have assigned parity");
                for &(neighbor, opposite_parity) in &neighbors[face] {
                    let expected = face_flipped ^ opposite_parity;
                    match flipped[neighbor] {
                        Some(actual) if actual != expected => {
                            return Err(GeometryError::NonOrientableMesh);
                        }
                        Some(_) => {}
                        None => {
                            flipped[neighbor] = Some(expected);
                            pending.push_back(neighbor);
                        }
                    }
                }
            }
        }

        let mut faces = self.faces.clone();
        let mut flipped_face_count = 0;
        for (face, flip) in faces.iter_mut().zip(flipped) {
            if flip.expect("every face component receives an orientation") {
                *face = face.reversed();
                flipped_face_count += 1;
            }
        }
        if flipped_face_count == 0 {
            return Ok((self.clone(), 0));
        }
        let mut unified = Self::from_validated_parts(self.vertices.clone(), faces)
            .rebuild_ngons_for_same_faces(&self.ngons)?;
        unified.vertex_colors = self.vertex_colors.clone();
        unified.vertex_channels = self.vertex_channels.clone();
        unified.recompute_stored_normals()?;
        Ok((unified, flipped_face_count))
    }
}
