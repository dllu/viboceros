//! Topology-edge splitting with explicit seam and output-order policies.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
enum SplitPosition {
    Middle,
    Last,
}

/// Every candidate contains two original vertices and exactly one split point.
/// Retain canonical insertion order separately from final face winding.
struct SplitTriangle {
    raw_vertices: [u32; 2],
    split_position: SplitPosition,
    forward: bool,
}

impl SplitTriangle {
    fn new(raw_vertices: [u32; 2], split_position: SplitPosition, forward: bool) -> Self {
        Self {
            raw_vertices,
            split_position,
            forward,
        }
    }

    fn canonical_vertices(&self) -> [Option<u32>; 3] {
        let [first, second] = self.raw_vertices.map(Some);
        match self.split_position {
            SplitPosition::Middle => [first, None, second],
            SplitPosition::Last => [first, second, None],
        }
    }

    fn oriented_face(&self, mut triangle: [u32; 3]) -> MeshFace {
        if !self.forward {
            triangle.swap(1, 2);
        }
        MeshFace::Triangle(triangle)
    }
}

/// Each incident triangle produces two candidates, and each quad produces
/// three, before endpoint-coincident candidates are removed.
fn replacement_count(triangles: usize, quads: usize) -> Result<usize, GeometryError> {
    triangles
        .checked_mul(2)
        .and_then(|triangles| {
            quads
                .checked_mul(3)
                .and_then(|quads| triangles.checked_add(quads))
        })
        .ok_or(GeometryError::TooManyMeshFaces)
}

/// Include the unused split point retained by the welded endpoint path.
fn output_vertex_count(
    retained: usize,
    generated: usize,
    welded: bool,
) -> Result<usize, GeometryError> {
    let added = if welded {
        1
    } else {
        generated
            .checked_mul(3)
            .ok_or(GeometryError::TooManyMeshVertices)?
    };
    let count = retained
        .checked_add(added)
        .ok_or(GeometryError::TooManyMeshVertices)?;
    if count
        .checked_sub(1)
        .is_some_and(|last| u32::try_from(last).is_err())
    {
        return Err(GeometryError::TooManyMeshVertices);
    }
    Ok(count)
}

impl TriangleMesh {
    /// Divides one exact-location topology edge at a normalized parameter.
    ///
    /// The parameter follows the direction returned by [`Self::wireframe_lines`]
    /// and values outside `[0, 1]` are rejected with `None`, matching
    /// RhinoCommon's `MeshTopologyEdgeList::SplitEdge`. Unaffected faces come
    /// first in source order and replacement triangles append in incident-face
    /// order. A welded edge shares one appended split vertex. Splitting an
    /// unwelded edge fully separates every replacement triangle, preserving
    /// Rhino's raw-vertex and seam behavior. Exact endpoint splits retain
    /// source triangles and append their coincident replacement.
    pub fn split_topology_edge(
        &self,
        edge_index: usize,
        parameter: Real,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let data = self.topology_data();
        let edge_count = data.edges.len();
        let Some((&(first_topology_vertex, second_topology_vertex), incidence)) =
            data.edges.iter().nth(edge_index)
        else {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: edge_index,
                edge_count,
            });
        };
        if !parameter.is_finite() || !(0.0..=1.0).contains(&parameter) {
            return Ok(None);
        }

        let first = data.topological_points[first_topology_vertex];
        let second = data.topological_points[second_topology_vertex];
        let edge = LineSegment::try_new(first, second, tolerance)?;
        let split_point = if parameter < 0.5 {
            edge.point_at(parameter)?
        } else {
            edge.reversed().point_at(1.0 - parameter)?
        };
        let split_at_endpoint = split_point == first || split_point == second;
        let first_raw_edge = incidence
            .uses()
            .next()
            .expect("a topology edge records at least one face use")
            .raw_vertices;
        let welded = incidence
            .uses()
            .all(|edge_use| edge_use.raw_vertices == first_raw_edge);
        let quad_count = incidence
            .uses()
            .filter(|edge_use| matches!(self.faces[edge_use.face], MeshFace::Quad(_)))
            .count();
        let candidate_count = replacement_count(incidence.count - quad_count, quad_count)?;
        let mut generated = Vec::<SplitTriangle>::new();
        generated
            .try_reserve_exact(candidate_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        let mut affected_faces = vec![false; self.faces.len()];
        for edge_use in incidence.uses() {
            affected_faces[edge_use.face] = true;
            let [from, to] = edge_use.raw_vertices;
            match self.faces[edge_use.face] {
                MeshFace::Triangle(indices) => {
                    let opposite = indices[(edge_use.side + 2) % 3];
                    generated.extend([
                        SplitTriangle::new([opposite, from], SplitPosition::Last, edge_use.forward),
                        SplitTriangle::new([opposite, to], SplitPosition::Middle, edge_use.forward),
                    ]);
                }
                MeshFace::Quad(indices) => {
                    let after_edge = indices[(edge_use.side + 2) % 4];
                    let before_edge = indices[(edge_use.side + 3) % 4];
                    let (from_opposite, to_opposite) = if edge_use.forward {
                        (before_edge, after_edge)
                    } else {
                        (after_edge, before_edge)
                    };
                    generated.extend([
                        SplitTriangle::new(
                            [from_opposite, to_opposite],
                            SplitPosition::Middle,
                            edge_use.forward,
                        ),
                        SplitTriangle::new(
                            [from_opposite, from],
                            SplitPosition::Last,
                            edge_use.forward,
                        ),
                        SplitTriangle::new(
                            [to_opposite, to],
                            SplitPosition::Middle,
                            edge_use.forward,
                        ),
                    ]);
                }
            }
        }
        debug_assert_eq!(generated.len(), candidate_count);
        generated.retain(|candidate| {
            let [a, b, c] = candidate
                .canonical_vertices()
                .map(|raw| raw.map_or(split_point, |raw| self.vertices[raw as usize]));
            a != b && b != c && c != a
        });

        let retained_faces = self
            .faces
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(face_index, face)| {
                (!affected_faces[face_index]
                    || (split_at_endpoint && matches!(face, MeshFace::Triangle(_))))
                .then_some(face)
            })
            .collect::<Vec<_>>();
        let mut used = vec![false; self.vertices.len()];
        for face in &retained_faces {
            for &raw in face.indices() {
                used[raw as usize] = true;
            }
        }
        if welded {
            for candidate in &generated {
                for raw in candidate.raw_vertices {
                    used[raw as usize] = true;
                }
            }
        }

        let retained_vertex_count = used.iter().filter(|&&retain| retain).count();
        let vertex_count = output_vertex_count(retained_vertex_count, generated.len(), welded)?;
        let face_count = retained_faces
            .len()
            .checked_add(generated.len())
            .ok_or(GeometryError::TooManyMeshFaces)?;
        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        let mut faces = Vec::new();
        faces
            .try_reserve_exact(face_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        let mut raw_remap = vec![0_u32; self.vertices.len()];
        for (raw, (&point, retain)) in self.vertices.iter().zip(used).enumerate() {
            if !retain {
                continue;
            }
            raw_remap[raw] =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            vertices.push(point);
        }
        faces.extend(
            retained_faces
                .into_iter()
                .map(|face| face.remapped(|raw| raw_remap[raw as usize])),
        );

        if welded {
            let split_vertex =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            vertices.push(split_point);
            faces.extend(generated.into_iter().map(|candidate| {
                let triangle = candidate
                    .canonical_vertices()
                    .map(|raw| raw.map_or(split_vertex, |raw| raw_remap[raw as usize]));
                candidate.oriented_face(triangle)
            }));
        } else {
            for candidate in generated {
                let mut triangle = [0_u32; 3];
                for (target, raw) in triangle.iter_mut().zip(candidate.canonical_vertices()) {
                    *target = u32::try_from(vertices.len())
                        .map_err(|_| GeometryError::TooManyMeshVertices)?;
                    vertices.push(raw.map_or(split_point, |raw| self.vertices[raw as usize]));
                }
                faces.push(candidate.oriented_face(triangle));
            }
        }
        Ok(Some(Self::try_new_faces(vertices, faces, tolerance)?))
    }
}
