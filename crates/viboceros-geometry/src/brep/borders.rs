//! Exact topological boundary chaining; no fitting or endpoint edits.
use super::*;
mod order;

impl Brep {
    /// Returns the selected face's exact non-seam boundary curves in connected
    /// components.
    ///
    /// Mated edges are included because they become naked when a single face
    /// is considered in isolation. Seam and singular trims are excluded. Trim
    /// order seeds chaining and curves follow face orientation. Closed linear
    /// loops start at their last endpoint join; disconnected linear components
    /// use surviving join-fragment order. No edge fitting or endpoint deformation
    /// occurs.
    pub fn face_boundary_curve_components(
        &self,
        face_index: usize,
    ) -> Result<Vec<Vec<NurbsCurve>>, GeometryError> {
        let face = self
            .faces
            .get(face_index)
            .ok_or(GeometryError::BrepFaceIndexOutOfRange {
                face: face_index,
                face_count: self.faces.len(),
            })?;
        let mut seen = BTreeSet::new();
        let edges = face
            .loops
            .iter()
            .flat_map(|face_loop| &face_loop.trims)
            .filter(|trim| matches!(trim.trim_type, BrepTrimType::Boundary | BrepTrimType::Mated))
            .filter_map(|trim| {
                let edge = trim.edge.expect("validated boundary trim has an edge");
                seen.insert(edge)
                    .then_some((edge, trim.reversed_3d ^ face.reversed))
            })
            .collect::<Vec<_>>();
        self.boundary_curve_components(&edges)
    }

    /// Exact connected chains of edges used by exactly one trim. Curves follow
    /// their incident face orientation; mated and seam edges are excluded.
    /// Traversal uses O(E log E) time and O(E) auxiliary storage.
    pub fn naked_boundary_curve_components(&self) -> Result<Vec<Vec<NurbsCurve>>, GeometryError> {
        let counts = self.edge_use_counts();
        let mut orientations = vec![None; self.edges.len()];
        for face in &self.faces {
            for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
                if let Some(edge) = trim.edge
                    && counts[edge] == 1
                {
                    orientations[edge] = Some(trim.reversed_3d ^ face.reversed);
                }
            }
        }
        let edges = orientations
            .into_iter()
            .enumerate()
            .filter_map(|(edge, reversed)| reversed.map(|reversed| (edge, reversed)))
            .collect::<Vec<_>>();
        self.boundary_curve_components(&edges)
    }

    fn boundary_curve_components(
        &self,
        ordered_edges: &[(usize, bool)],
    ) -> Result<Vec<Vec<NurbsCurve>>, GeometryError> {
        if ordered_edges.is_empty() {
            return Ok(Vec::new());
        }

        let mut local_edges_at_vertex = BTreeMap::<usize, Vec<usize>>::new();
        for (local_edge, &(edge_index, _)) in ordered_edges.iter().enumerate() {
            let vertices = self.edges[edge_index].vertices;
            local_edges_at_vertex
                .entry(vertices[0])
                .or_default()
                .push(local_edge);
            if vertices[1] != vertices[0] {
                local_edges_at_vertex
                    .entry(vertices[1])
                    .or_default()
                    .push(local_edge);
            }
        }

        let mut visited = vec![false; ordered_edges.len()];
        let mut visited_vertices = BTreeSet::new();
        let line_ranks = ordered_edges
            .iter()
            .all(|&(edge, _)| {
                self.edges[edge].curve.degree() == 1
                    && self.edges[edge].curve.control_points().len() == 2
            })
            .then(|| {
                let ends = ordered_edges
                    .iter()
                    .map(|&(edge, reversed)| oriented_edge_vertices(&self.edges[edge], reversed))
                    .collect::<Vec<_>>();
                order::linear_component_ranks(&ends, &local_edges_at_vertex)
            })
            .flatten();
        let mut components = Vec::new();
        for root in 0..ordered_edges.len() {
            if visited[root] {
                continue;
            }
            visited[root] = true;
            let mut pending = vec![root];
            let mut component = Vec::new();
            while let Some(local_edge) = pending.pop() {
                component.push(local_edge);
                for vertex in self.edges[ordered_edges[local_edge].0].vertices {
                    if !visited_vertices.insert(vertex) {
                        continue;
                    }
                    for &neighbor in &local_edges_at_vertex[&vertex] {
                        if !visited[neighbor] {
                            visited[neighbor] = true;
                            pending.push(neighbor);
                        }
                    }
                }
            }
            component.sort_unstable();
            components.push((
                line_ranks.as_ref().map_or(root, |ranks| ranks[root]),
                self.chain_boundary_component(&component, ordered_edges)?,
            ));
        }
        components.sort_by_key(|(rank, _)| *rank);
        Ok(components.into_iter().map(|(_, curves)| curves).collect())
    }

    fn chain_boundary_component(
        &self,
        edge_indices: &[usize],
        ordered_edges: &[(usize, bool)],
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        debug_assert!(!edge_indices.is_empty());
        let mut chain = VecDeque::with_capacity(edge_indices.len());
        chain.push_back((edge_indices[0], ordered_edges[edge_indices[0]].1));
        let mut at_vertex = BTreeMap::<usize, BTreeSet<usize>>::new();
        for &edge in &edge_indices[1..] {
            for vertex in self.edges[ordered_edges[edge].0].vertices {
                at_vertex.entry(vertex).or_default().insert(edge);
            }
        }
        while chain.len() < edge_indices.len() {
            let (first_edge, first_reversed) = chain[0];
            let (last_edge, last_reversed) = chain[chain.len() - 1];
            let first_vertices =
                oriented_edge_vertices(&self.edges[ordered_edges[first_edge].0], first_reversed);
            let last_vertices =
                oriented_edge_vertices(&self.edges[ordered_edges[last_edge].0], last_reversed);
            // The least unused edge touching either end is the same choice
            // as a sorted full scan, but costs O(log E) per appended edge.
            let candidate = [first_vertices[0], last_vertices[1]]
                .into_iter()
                .filter_map(|vertex| {
                    at_vertex
                        .get(&vertex)
                        .and_then(|edges| edges.first())
                        .copied()
                })
                .min();
            let Some(edge) = candidate else {
                return Err(GeometryError::InvalidBrepTopology {
                    context: "a boundary edge component could not be chained",
                });
            };
            let vertices = self.edges[ordered_edges[edge].0].vertices;
            let placement = if vertices[0] == last_vertices[1] {
                (false, false)
            } else if vertices[1] == last_vertices[1] {
                (false, true)
            } else if vertices[1] == first_vertices[0] {
                (true, false)
            } else {
                (true, true)
            };
            for vertex in vertices {
                at_vertex.get_mut(&vertex).unwrap().remove(&edge);
            }
            let (prepend, reversed) = placement;
            if prepend {
                chain.push_front((edge, reversed));
            } else {
                chain.push_back((edge, reversed));
            }
        }
        if chain.len() > 1 {
            let (first_edge, first_reversed) = chain[0];
            let (last_edge, last_reversed) = chain[chain.len() - 1];
            let first =
                oriented_edge_vertices(&self.edges[ordered_edges[first_edge].0], first_reversed)[0];
            let last =
                oriented_edge_vertices(&self.edges[ordered_edges[last_edge].0], last_reversed)[1];
            if first == last {
                let mut start_position = chain
                    .iter()
                    .position(|(edge, _)| *edge == edge_indices[0])
                    .expect("the boundary chain must retain its root edge");
                let linear = edge_indices.iter().all(|&edge| {
                    let curve = &self.edges[ordered_edges[edge].0].curve;
                    curve.degree() == 1 && curve.control_points().len() == 2
                });
                if linear {
                    // Exact two-valent loops have one possible join per
                    // vertex. Pairwise line joining visits those joins by
                    // (larger input index, smaller input index). The last
                    // connection closes the loop without moving its seam.
                    // Thus its vertex, not a fixed offset from the seed, is
                    // the start. No quadratic pair enumeration is needed.
                    start_position = (0..chain.len())
                        .max_by_key(|&i| {
                            let left = chain[(i + chain.len() - 1) % chain.len()].0;
                            let right = chain[i].0;
                            (left.max(right), left.min(right))
                        })
                        .unwrap();
                }
                chain.rotate_left(start_position);
            }
        }
        chain
            .into_iter()
            .map(|(edge, reversed)| {
                if reversed {
                    self.edges[ordered_edges[edge].0].curve.reversed()
                } else {
                    Ok(self.edges[ordered_edges[edge].0].curve.clone())
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naked_chains_retain_every_exact_edge_once_and_omit_all_shared_edges() {
        let tolerance = Tolerance::DEFAULT;
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            tolerance,
        )
        .unwrap();
        let solid = Brep::try_box(frame, [[0., 4.], [0., 3.], [0., 5.]], tolerance).unwrap();
        for faces in [
            vec![0],
            vec![0, 1],
            vec![2, 5],
            vec![0, 2, 3, 4, 5],
            vec![0, 1, 2, 3, 4, 5],
        ] {
            let source = solid.duplicate_faces(&faces, tolerance).unwrap();
            let mut expected = source
                .edges
                .iter()
                .zip(source.edge_use_counts())
                .filter(|(_, count)| *count == 1)
                .map(|(edge, _)| edge.curve.clone())
                .collect::<Vec<_>>();
            for chain in source.naked_boundary_curve_components().unwrap() {
                assert!(!chain.is_empty());
                for (curve, next) in chain.iter().zip(chain.iter().cycle().skip(1)) {
                    assert_eq!(
                        curve.evaluate(*curve.domain().end()).unwrap(),
                        next.evaluate(*next.domain().start()).unwrap()
                    );
                    let reversed = curve.reversed().unwrap();
                    let index = expected
                        .iter()
                        .position(|original| original == curve || *original == reversed)
                        .expect("an original naked edge exactly once");
                    expected.swap_remove(index);
                }
            }
            assert!(expected.is_empty(), "{faces:?}");
        }
    }
}
