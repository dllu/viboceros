//! Copy and remap topology after faces or individual loops are removed.
use super::incidence::{
    accumulate_edge_uses, classify_edge_trim, clear_edge_uses, count_edge_uses,
};
use super::*;

pub(super) struct FaceSubset {
    pub(super) vertices: Vec<BrepVertex>,
    pub(super) edges: Vec<BrepEdge>,
    pub(super) faces: Vec<BrepFace>,
}

impl Brep {
    /// Duplicates a non-empty, unique face subset as one validated B-rep.
    ///
    /// Faces retain the requested order while vertices and edges retain source
    /// order. Edges mated only to omitted faces become boundaries, seams remain
    /// seams, and vertex tolerances are recomputed like OpenNURBS
    /// `ON_Brep::DuplicateFaces`.
    pub fn duplicate_faces(
        &self,
        face_indices: &[usize],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.copy_face_subset(face_indices, tolerance, true)
    }

    /// Copies a non-empty, unique face subset while preserving source vertex
    /// tolerances, matching OpenNURBS `ON_Brep::SubBrep` and the remainder left
    /// by Rhino's `ExtractSrf` command.
    pub fn sub_brep(
        &self,
        face_indices: &[usize],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.copy_face_subset(face_indices, tolerance, false)
    }

    fn copy_face_subset(
        &self,
        face_indices: &[usize],
        tolerance: Tolerance,
        recompute_vertex_tolerances: bool,
    ) -> Result<Self, GeometryError> {
        if face_indices.is_empty() {
            return Err(GeometryError::EmptyBrepFaceSubset);
        }
        let mut selected_faces = vec![false; self.faces.len()];
        for &face in face_indices {
            let Some(selected) = selected_faces.get_mut(face) else {
                return Err(GeometryError::BrepFaceIndexOutOfRange {
                    face,
                    face_count: self.faces.len(),
                });
            };
            if std::mem::replace(selected, true) {
                return Err(GeometryError::DuplicateBrepFaceIndex { face });
            }
        }

        let faces = face_indices
            .iter()
            .map(|&index| self.faces[index].clone())
            .collect();
        let FaceSubset {
            mut vertices,
            mut edges,
            faces,
        } = self.compact_face_topology(faces);
        // Face copies use fresh NURBS edge records. Other topology edits
        // preserve the surviving edge's analytic provenance.
        for edge in &mut edges {
            edge.arc_representation = false;
        }
        if recompute_vertex_tolerances {
            recompute_duplicated_face_vertex_tolerances(&mut vertices, &edges, &faces)?;
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Duplicates every face as an independent, validated one-face B-rep.
    ///
    /// Source-order vertices and edges used by each face are compacted and
    /// remapped. Edges formerly mated to another face become boundaries,
    /// while multiple uses within the same loop remain seams. This matches
    /// the topology produced by Rhino's `BrepFace.DuplicateFace`/`Explode`
    /// path without approximating trims or underlying surfaces.
    pub fn explode_faces(&self, tolerance: Tolerance) -> Result<Vec<Self>, GeometryError> {
        (0..self.faces.len())
            .map(|face| self.duplicate_faces(&[face], tolerance))
            .collect()
    }

    /// Retained trims must reference the supplied topology, with the same
    /// endpoint indices as their spatial edges. Faces may omit loops and can
    /// be supplied in a different order.
    pub(super) fn compact_face_topology(&self, mut faces: Vec<BrepFace>) -> FaceSubset {
        let edge_uses = count_edge_uses(&faces, self.edges.len());
        let mut used_vertices = vec![false; self.vertices.len()];
        for boundary in faces.iter().flat_map(|face| &face.loops) {
            for trim in &boundary.trims {
                for vertex in trim.vertices {
                    used_vertices[vertex] = true;
                }
            }
        }

        let mut vertex_map = vec![usize::MAX; self.vertices.len()];
        let mut vertices = Vec::new();
        for (index, &vertex) in self.vertices.iter().enumerate() {
            if used_vertices[index] {
                vertex_map[index] = vertices.len();
                vertices.push(vertex);
            }
        }

        let mut edge_map = vec![usize::MAX; self.edges.len()];
        let mut edges = Vec::new();
        for (index, edge) in self.edges.iter().enumerate() {
            if edge_uses[index] != 0 {
                edge_map[index] = edges.len();
                let mut edge = edge.clone();
                edge.vertices = edge.vertices.map(|vertex| vertex_map[vertex]);
                edges.push(edge);
            }
        }

        let mut loop_uses = vec![0; self.edges.len()];
        for boundary in faces.iter_mut().flat_map(|face| &mut face.loops) {
            accumulate_edge_uses(boundary, &mut loop_uses);
            for trim in &mut boundary.trims {
                if let Some(edge) = trim.edge {
                    trim.trim_type = classify_edge_trim(edge_uses[edge], loop_uses[edge]);
                }
                trim.vertices = trim.vertices.map(|vertex| vertex_map[vertex]);
            }
            clear_edge_uses(boundary, &mut loop_uses);
            for trim in &mut boundary.trims {
                trim.edge = trim.edge.map(|edge| edge_map[edge]);
            }
        }
        FaceSubset {
            vertices,
            edges,
            faces,
        }
    }
}

fn recompute_duplicated_face_vertex_tolerances(
    vertices: &mut [BrepVertex],
    edges: &[BrepEdge],
    faces: &[BrepFace],
) -> Result<(), GeometryError> {
    let mut distances = vec![0.0_f64; vertices.len()];
    for edge in edges {
        let domain = edge.curve.domain();
        for (vertex, parameter) in edge
            .vertices
            .into_iter()
            .zip([*domain.start(), *domain.end()])
        {
            let endpoint = edge.curve.evaluate(parameter)?;
            let distance = vertices[vertex].point.distance_to(endpoint)?;
            distances[vertex] = distances[vertex].max(distance);
        }
    }
    for face in faces {
        for trim in face.loops.iter().flat_map(|boundary| &boundary.trims) {
            if trim.edge.is_some() {
                let parameters = [trim.curve.start_point()?, trim.curve.end_point()?];
                for (vertex, parameter) in trim.vertices.into_iter().zip(parameters) {
                    let endpoint = face.surface.evaluate(parameter.x(), parameter.y())?;
                    let distance = vertices[vertex].point.distance_to(endpoint)?;
                    distances[vertex] = distances[vertex].max(distance);
                }
            }
        }
    }
    for (vertex, distance) in vertices.iter_mut().zip(distances) {
        vertex.tolerance = if distance <= CURVE_COINCIDENCE_ABSOLUTE {
            0.0
        } else {
            let expanded = distance * 1.001;
            require_finite([expanded], "duplicated B-rep face vertex tolerance")?;
            expanded
        };
    }
    Ok(())
}
