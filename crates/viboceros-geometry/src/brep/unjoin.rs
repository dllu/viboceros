//! Exact edge separation and local vertex-fan splitting, without refitting.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Usage {
    face: usize,
    corner: usize,
}

impl Brep {
    /// Separates selected joined edges and returns edge-connected components.
    /// Empty, naked-only, and seam-only selections return an empty vector.
    /// Duplicate indices are processed once, in their first-occurrence order.
    /// Every index is checked before editing; input is never mutated.
    ///
    /// Edges shared by different faces are copied once per face; seams within
    /// a face remain joined. Endpoint vertices split only when cutting edges
    /// disconnects their local corner fans. Coincident points are never welded.
    /// Surfaces, UV curves, 3D curves, orientation and tolerances remain exact.
    /// Components and faces follow source face order; edges retain source order
    /// followed by copies in selection order. Vertices follow first trim use.
    pub fn try_unjoin_edges(
        &self,
        selected: &[usize],
        tolerance: Tolerance,
    ) -> Result<Vec<Self>, GeometryError> {
        if selected.len() > 100_000 {
            return invalid("too many B-rep edges to separate");
        }
        let mut requested = vec![false; self.edges.len()];
        let mut ordered = Vec::new();
        for &edge in selected {
            let Some(seen) = requested.get_mut(edge) else {
                return invalid("edge separation references a missing edge");
            };
            if !std::mem::replace(seen, true) {
                ordered.push(edge);
            }
        }
        if ordered.is_empty() {
            return Ok(Vec::new());
        }
        let mut offsets = Vec::with_capacity(self.faces.len());
        let mut corner_vertices = Vec::new();
        let mut uses = vec![Vec::<Usage>::new(); self.edges.len()];
        for (face, record) in self.faces.iter().enumerate() {
            let mut loops = Vec::new();
            for boundary in &record.loops {
                let base = corner_vertices.len();
                loops.push(base);
                for (trim_index, trim) in boundary.trims.iter().enumerate() {
                    let corner = base + trim_index;
                    corner_vertices.push(trim.vertices[0]);
                    if let Some(edge) = trim.edge {
                        uses[edge].push(Usage { face, corner });
                    }
                }
            }
            offsets.push(loops);
        }
        ordered.retain(|&edge| {
            uses[edge]
                .first()
                .zip(uses[edge].last())
                .is_some_and(|(a, b)| a.face != b.face)
        });
        if ordered.is_empty() {
            return Ok(Vec::new());
        }
        let mut touched = vec![false; self.vertices.len()];
        // Scratch tables contain indices only. Clone geometry once, when each
        // final component is built, rather than copying the complete source.
        let mut edge_sources = (0..self.edges.len()).collect::<Vec<_>>();
        requested.fill(false);
        let mut corner_edges = vec![usize::MAX; corner_vertices.len()];
        for (edge, usages) in uses.iter().enumerate() {
            for usage in usages {
                corner_edges[usage.corner] = edge;
            }
        }
        for edge in ordered {
            requested[edge] = true;
            for &vertex in &self.edges[edge].vertices {
                touched[vertex] = true;
            }
            let mut previous = usize::MAX;
            let mut replacement = edge;
            for usage in &uses[edge] {
                if usage.face != previous {
                    if previous != usize::MAX {
                        replacement = edge_sources.len();
                        edge_sources.push(edge);
                    }
                    previous = usage.face;
                }
                corner_edges[usage.corner] = replacement;
            }
        }
        // Each corner is a loop junction. Sharing a retained edge joins its
        // corresponding endpoint corners; a cut edge only joins its own face.
        let mut fans = Fans::new(corner_vertices.len());
        let mut original_corner = vec![usize::MAX; self.vertices.len()];
        for (corner, &vertex) in corner_vertices.iter().enumerate() {
            if !touched[vertex] {
                if original_corner[vertex] == usize::MAX {
                    original_corner[vertex] = corner;
                } else {
                    fans.join(original_corner[vertex], corner);
                }
            }
        }
        let mut edge_corners = vec![None::<[usize; 2]>; edge_sources.len()];
        let mut edge_counts = vec![0usize; edge_sources.len()];
        for (face, record) in self.faces.iter().enumerate() {
            for (boundary_index, boundary) in record.loops.iter().enumerate() {
                let base = offsets[face][boundary_index];
                let count = boundary.trims.len();
                for (index, trim) in boundary.trims.iter().enumerate() {
                    let corner = base + index;
                    let next = base + (index + 1) % count;
                    if corner_vertices[corner] == corner_vertices[next] {
                        fans.join(corner, next);
                    }
                    if trim.edge.is_some() {
                        let edge = corner_edges[corner];
                        edge_counts[edge] += 1;
                        let ends = if trim.reversed_3d {
                            [next, corner]
                        } else {
                            [corner, next]
                        };
                        if let Some(first) = edge_corners[edge] {
                            fans.join(first[0], ends[0]);
                            fans.join(first[1], ends[1]);
                        } else {
                            edge_corners[edge] = Some(ends);
                        }
                    }
                }
            }
        }
        let components = self.edge_connected_face_components_where(|edge| !requested[edge]);
        let mut face_component = vec![usize::MAX; self.faces.len()];
        for (part, faces) in components.iter().enumerate() {
            for &face in faces {
                face_component[face] = part;
            }
        }
        let mut part_edges = vec![Vec::new(); components.len()];
        let mut first_face = vec![usize::MAX; edge_sources.len()];
        for (face, record) in self.faces.iter().enumerate() {
            for (boundary_index, boundary) in record.loops.iter().enumerate() {
                let base = offsets[face][boundary_index];
                for (index, trim) in boundary.trims.iter().enumerate() {
                    if trim.edge.is_some() {
                        first_face[corner_edges[base + index]] = face;
                    }
                }
            }
        }
        for (edge, &face) in first_face.iter().enumerate() {
            part_edges[face_component[face]].push(edge);
        }
        let mut vertex_map = vec![usize::MAX; corner_vertices.len()];
        let mut edge_map = vec![usize::MAX; edge_sources.len()];
        let mut parts = Vec::with_capacity(components.len());
        for (part, indices) in components.into_iter().enumerate() {
            let mut used_fans = Vec::new();
            let mut vertices = Vec::new();
            let mut faces = Vec::with_capacity(indices.len());
            for face in indices {
                let mut record = self.faces[face].clone();
                for (boundary_index, boundary) in record.loops.iter_mut().enumerate() {
                    let base = offsets[face][boundary_index];
                    let count = boundary.trims.len();
                    for (index, trim) in boundary.trims.iter_mut().enumerate() {
                        if trim.edge.is_some() {
                            let edge = corner_edges[base + index];
                            trim.edge = Some(edge);
                            if edge_counts[edge] == 1 {
                                trim.trim_type = BrepTrimType::Boundary;
                            }
                        }
                        for (end, corner) in [base + index, base + (index + 1) % count]
                            .into_iter()
                            .enumerate()
                        {
                            let root = fans.root(corner);
                            if vertex_map[root] == usize::MAX {
                                vertex_map[root] = vertices.len();
                                vertices.push(self.vertices[corner_vertices[corner]]);
                                used_fans.push(root);
                            }
                            trim.vertices[end] = vertex_map[root];
                        }
                    }
                }
                faces.push(record);
            }
            let mut edges = Vec::with_capacity(part_edges[part].len());
            for &global in &part_edges[part] {
                edge_map[global] = edges.len();
                let mut edge = self.edges[edge_sources[global]].clone();
                edge.vertices = edge_corners[global]
                    .unwrap()
                    .map(|corner| vertex_map[fans.root(corner)]);
                edges.push(edge);
            }
            for face in &mut faces {
                for trim in face
                    .loops
                    .iter_mut()
                    .flat_map(|boundary| &mut boundary.trims)
                {
                    if let Some(edge) = trim.edge {
                        trim.edge = Some(edge_map[edge]);
                    }
                }
            }
            parts.push(Self::try_new(vertices, edges, faces, tolerance)?);
            for root in used_fans {
                vertex_map[root] = usize::MAX;
            }
        }
        Ok(parts)
    }
}

struct Fans {
    parents: Vec<usize>,
    ranks: Vec<u8>,
}
impl Fans {
    fn new(count: usize) -> Self {
        Self {
            parents: (0..count).collect(),
            ranks: vec![0; count],
        }
    }
    fn root(&mut self, mut index: usize) -> usize {
        while self.parents[index] != index {
            self.parents[index] = self.parents[self.parents[index]];
            index = self.parents[index];
        }
        index
    }
    fn join(&mut self, a: usize, b: usize) {
        let (mut a, mut b) = (self.root(a), self.root(b));
        if a == b {
            return;
        }
        if self.ranks[a] < self.ranks[b] {
            std::mem::swap(&mut a, &mut b);
        }
        self.parents[b] = a;
        if self.ranks[a] == self.ranks[b] {
            self.ranks[a] += 1;
        }
    }
}
