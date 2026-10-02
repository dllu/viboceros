//! Exact boundary restoration and interior-hole removal.
use super::*;

#[cfg(test)]
mod tests;

/// Validated hole-removal geometry and the original topology it removed.
/// Opening indices refer only to surviving faces; wall indices refer to deleted
/// source faces. Callers can retain exact trim curves or detached wall geometry
/// without guessing the traversal from differences between compacted tables.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepHoleRemoval {
    brep: Brep,
    removed_faces: Vec<usize>,
    removed_openings: Vec<(usize, usize)>,
}

impl BrepHoleRemoval {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn into_brep(self) -> Brep {
        self.brep
    }

    pub fn removed_faces(&self) -> &[usize] {
        &self.removed_faces
    }

    pub fn removed_openings(&self) -> &[(usize, usize)] {
        &self.removed_openings
    }
}

impl Brep {
    /// Removes all interior holes, including faces joined to their boundaries.
    /// A source without holes is returned as an unchanged independent copy.
    pub fn try_remove_all_holes(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let holes = self
            .faces
            .iter()
            .enumerate()
            .flat_map(|(face, record)| {
                (1..record.loops.len()).map(move |boundary| (face, boundary))
            })
            .collect::<Vec<_>>();
        if holes.is_empty() {
            return Ok(Some(self.clone()));
        }
        self.try_remove_holes(&holes, tolerance)
    }

    /// Removes the selected interior loops and any connected hole walls.
    /// Indices identify a face and its local loop; outer loops are ignored.
    /// Repeated indices select a hole once. Invalid indices return an error
    /// before any geometry changes. A selection without inner loops returns
    /// `None`.
    ///
    /// Traversal stops at neighboring inner loops, which are also removed.
    /// Faces reached through their outer loops belong to the hole walls and
    /// are deleted. This closes both ends of a joined through hole when only
    /// one opening is selected. Surviving control nets, domains, tolerances,
    /// and orientation are preserved; unused topology is compacted in source
    /// order, and the complete result is validated.
    pub fn try_remove_holes(
        &self,
        holes: &[(usize, usize)],
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        Ok(self
            .try_remove_holes_with_topology(holes, tolerance)?
            .map(BrepHoleRemoval::into_brep))
    }

    /// The selected-hole operation with original wall and opening indices.
    /// Indices are unique and ordered by their original face/loop table order.
    /// Validation and no-op behavior match [`Self::try_remove_holes`].
    pub fn try_remove_holes_with_topology(
        &self,
        holes: &[(usize, usize)],
        tolerance: Tolerance,
    ) -> Result<Option<BrepHoleRemoval>, GeometryError> {
        let mut removed = self
            .faces
            .iter()
            .map(|face| vec![false; face.loops.len()])
            .collect::<Vec<_>>();
        let mut queue = Vec::new();
        for &(face, boundary) in holes {
            let Some(record) = self.faces.get(face) else {
                return Err(GeometryError::BrepFaceIndexOutOfRange {
                    face,
                    face_count: self.faces.len(),
                });
            };
            let Some(boundary_record) = record.loops.get(boundary) else {
                return Err(GeometryError::InvalidBrepTopology {
                    context: "hole loop index outside face",
                });
            };
            if boundary_record.loop_type == BrepLoopType::Inner
                && !std::mem::replace(&mut removed[face][boundary], true)
            {
                queue.push((face, boundary));
            }
        }
        if queue.is_empty() {
            return Ok(None);
        }
        let mut uses = vec![Vec::new(); self.edges.len()];
        for usage in self.trim_uses() {
            if let Some(edge) = usage.trim.edge {
                uses[edge].push((usage.face, usage.face_loop));
            }
        }
        let mut deleted_faces = vec![false; self.faces.len()];
        while let Some((face, boundary)) = queue.pop() {
            for trim in &self.faces[face].loops[boundary].trims {
                let Some(edge) = trim.edge else { continue };
                for &(neighbor, neighbor_loop) in &uses[edge] {
                    if (neighbor, neighbor_loop) == (face, boundary)
                        || deleted_faces[neighbor]
                        || removed[neighbor][neighbor_loop]
                    {
                        continue;
                    }
                    if self.faces[neighbor].loops[neighbor_loop].loop_type == BrepLoopType::Inner {
                        removed[neighbor][neighbor_loop] = true;
                        queue.push((neighbor, neighbor_loop));
                    } else {
                        deleted_faces[neighbor] = true;
                        for (boundary, marked) in removed[neighbor].iter_mut().enumerate() {
                            *marked = true;
                            queue.push((neighbor, boundary));
                        }
                    }
                }
            }
        }
        if deleted_faces.iter().all(|deleted| *deleted) {
            return Ok(None);
        }
        let faces = self
            .faces
            .iter()
            .enumerate()
            .filter(|(index, _)| !deleted_faces[*index])
            .map(|(index, face)| {
                let mut face = face.clone();
                face.loops = face
                    .loops
                    .into_iter()
                    .enumerate()
                    .filter(|(boundary, _)| !removed[index][*boundary])
                    .map(|(_, boundary)| boundary)
                    .collect();
                face
            })
            .collect::<Vec<_>>();
        let brep = compact_retained_faces(self, faces, tolerance)?;
        let removed_faces = deleted_faces
            .iter()
            .enumerate()
            .filter_map(|(face, deleted)| deleted.then_some(face))
            .collect();
        let removed_openings = removed
            .iter()
            .enumerate()
            .filter(|(face, _)| !deleted_faces[*face])
            .flat_map(|(face, loops)| {
                loops
                    .iter()
                    .enumerate()
                    .filter_map(move |(boundary, removed)| removed.then_some((face, boundary)))
            })
            .collect();
        Ok(Some(BrepHoleRemoval {
            brep,
            removed_faces,
            removed_openings,
        }))
    }

    /// Wraps a surface as one natural face using its original U/V intervals
    /// on spatial boundary curves. Reversed north/west curves use negated
    /// intervals. Control nets and UV trims remain exact; closed directions
    /// share seam edges and collapsed sides have singular trims.
    pub fn try_surface_face_with_native_edge_parameters(
        surface: NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        natural_untrimmed_face(surface, false, tolerance)
    }

    /// Removes every trim of one standalone face, restoring its complete
    /// underlying surface with native boundary-edge parameter intervals.
    /// The surface control net, knots, domain, and face orientation are exact.
    pub fn try_untrim_all(&self, tolerance: Tolerance) -> Result<Self, GeometryError> {
        let [face] = self.faces.as_slice() else {
            return Err(GeometryError::InvalidBrepTopology {
                context: "trim removal requires one standalone face",
            });
        };
        natural_untrimmed_face(face.surface.clone(), face.reversed, tolerance)
    }

    /// Restores the natural outer boundary of a standalone face and preserves
    /// every inner loop, underlying surface, and face sense exactly.
    ///
    /// Unused source vertices and edges are compacted in source order. Natural
    /// boundary topology is appended after the retained hole topology; no
    /// coincident vertices are welded and no curves are fitted. Spatial and UV
    /// tolerances on retained geometry are preserved. Closed directions and
    /// collapsed sides use the natural face constructor's seams and singular
    /// trims. The complete result is validated before it is returned.
    ///
    /// A joined face must first be detached by the caller: changing its outer
    /// boundary in place would also require editing its neighboring faces.
    pub fn try_untrim_outer_boundary(&self, tolerance: Tolerance) -> Result<Self, GeometryError> {
        if self.faces.len() != 1 {
            return Err(GeometryError::InvalidBrepTopology {
                context: "outer boundary restoration requires one standalone face",
            });
        }
        let face = &self.faces[0];
        let mut natural = natural_untrimmed_face(face.surface.clone(), face.reversed, tolerance)?;
        if face.loops.len() == 1 {
            return Ok(natural);
        }

        let mut used_vertices = vec![false; self.vertices.len()];
        let mut edge_uses = vec![0usize; self.edges.len()];
        for boundary in &face.loops[1..] {
            for trim in &boundary.trims {
                for vertex in trim.vertices {
                    used_vertices[vertex] = true;
                }
                if let Some(edge) = trim.edge {
                    edge_uses[edge] += 1;
                    for vertex in self.edges[edge].vertices {
                        used_vertices[vertex] = true;
                    }
                }
            }
        }
        let mut vertex_map = vec![usize::MAX; self.vertices.len()];
        let mut vertices = Vec::new();
        for (source, vertex) in self.vertices.iter().copied().enumerate() {
            if used_vertices[source] {
                vertex_map[source] = vertices.len();
                vertices.push(vertex);
            }
        }
        let mut edge_map = vec![usize::MAX; self.edges.len()];
        let mut edges = Vec::new();
        for (source, edge) in self.edges.iter().enumerate() {
            if edge_uses[source] > 0 {
                edge_map[source] = edges.len();
                let mut edge = edge.clone();
                edge.vertices = edge.vertices.map(|vertex| vertex_map[vertex]);
                edges.push(edge);
            }
        }
        let mut holes = Vec::with_capacity(face.loops.len() - 1);
        for boundary in &face.loops[1..] {
            let mut same_loop_uses = BTreeMap::new();
            for trim in &boundary.trims {
                if let Some(edge) = trim.edge {
                    *same_loop_uses.entry(edge).or_insert(0usize) += 1;
                }
            }
            let mut boundary = boundary.clone();
            for trim in &mut boundary.trims {
                trim.vertices = trim.vertices.map(|vertex| vertex_map[vertex]);
                if let Some(edge) = trim.edge {
                    trim.trim_type = if edge_uses[edge] == 1 {
                        BrepTrimType::Boundary
                    } else if same_loop_uses[&edge] >= 2 {
                        BrepTrimType::Seam
                    } else {
                        BrepTrimType::Mated
                    };
                    trim.edge = Some(edge_map[edge]);
                }
            }
            holes.push(boundary);
        }
        let vertex_offset = vertices.len();
        let edge_offset = edges.len();
        for edge in &mut natural.edges {
            edge.vertices = edge.vertices.map(|vertex| vertex + vertex_offset);
        }
        for trim in &mut natural.faces[0].loops[0].trims {
            trim.vertices = trim.vertices.map(|vertex| vertex + vertex_offset);
            trim.edge = trim.edge.map(|edge| edge + edge_offset);
        }
        vertices.append(&mut natural.vertices);
        edges.append(&mut natural.edges);
        natural.faces[0].loops.append(&mut holes);
        Self::try_new(vertices, edges, natural.faces, tolerance)
    }
}

fn compact_retained_faces(
    source: &Brep,
    mut faces: Vec<BrepFace>,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let mut used_vertices = vec![false; source.vertices.len()];
    let mut edge_uses = vec![0usize; source.edges.len()];
    for face in faces.iter() {
        for boundary in &face.loops {
            for trim in &boundary.trims {
                for vertex in trim.vertices {
                    used_vertices[vertex] = true;
                }
                if let Some(edge) = trim.edge {
                    edge_uses[edge] += 1;
                    for vertex in source.edges[edge].vertices {
                        used_vertices[vertex] = true;
                    }
                }
            }
        }
    }
    let mut vertex_map = vec![usize::MAX; source.vertices.len()];
    let mut vertices = Vec::new();
    for (index, vertex) in source.vertices.iter().copied().enumerate() {
        if used_vertices[index] {
            vertex_map[index] = vertices.len();
            vertices.push(vertex);
        }
    }
    let mut edge_map = vec![usize::MAX; source.edges.len()];
    let mut edges = Vec::new();
    for (index, edge) in source.edges.iter().enumerate() {
        if edge_uses[index] != 0 {
            edge_map[index] = edges.len();
            let mut edge = edge.clone();
            edge.vertices = edge.vertices.map(|vertex| vertex_map[vertex]);
            edges.push(edge);
        }
    }
    for face in faces.iter_mut() {
        for boundary in &mut face.loops {
            let mut same_loop_uses = BTreeMap::new();
            for trim in &boundary.trims {
                if let Some(edge) = trim.edge {
                    *same_loop_uses.entry(edge).or_insert(0usize) += 1;
                }
            }
            for trim in &mut boundary.trims {
                trim.vertices = trim.vertices.map(|vertex| vertex_map[vertex]);
                if let Some(edge) = trim.edge {
                    trim.trim_type = if edge_uses[edge] == 1 {
                        BrepTrimType::Boundary
                    } else if same_loop_uses[&edge] >= 2 {
                        BrepTrimType::Seam
                    } else {
                        BrepTrimType::Mated
                    };
                    trim.edge = Some(edge_map[edge]);
                }
            }
        }
    }
    Brep::try_new(vertices, edges, faces, tolerance)
}

// Editing an existing outer loop uses native surface intervals for its new
// isocurves (north/west intervals are negated by curve reversal). Natural
// insertion's local edge intervals remain useful elsewhere, but Rhino's
// public UntrimAll/UntrimBorder outputs retain the native intervals on shifted UVs.
fn natural_untrimmed_face(
    surface: NurbsSurface,
    reversed: bool,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let domains = [surface.domain_u(), surface.domain_v()];
    let mut natural = Brep::try_rectangular_surface_face_with_orientation(
        surface,
        domains[0].clone(),
        domains[1].clone(),
        reversed,
        tolerance,
    )?;
    let mut visited = [false; 4];
    let mut changed = false;
    for (side, trim) in natural.faces[0].loops[0].trims.iter().enumerate() {
        let Some(edge) = trim.edge else { continue };
        if std::mem::replace(&mut visited[edge], true) {
            continue;
        }
        let axis = &domains[side % 2];
        let domain = if side < 2 {
            axis.clone()
        } else {
            -*axis.end()..=-*axis.start()
        };
        if natural.edges[edge].curve.domain() != domain {
            natural.edges[edge].curve = natural.edges[edge].curve.try_reparameterized(domain)?;
            changed = true;
        }
    }
    if changed {
        natural.validate(tolerance)?;
    }
    Ok(natural)
}
