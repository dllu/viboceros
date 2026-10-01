//! Exact outer-boundary replacement while retaining interior trim geometry.
use super::*;

#[cfg(test)]
mod tests;

impl Brep {
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
