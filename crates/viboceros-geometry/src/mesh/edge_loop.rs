//! Continuation through ordinary polygon vertices, in stable topology order.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshEdgeLoop {
    pub edges: Vec<usize>,
    /// Exact-location topology vertex indices; closed paths repeat their start.
    pub vertices: Vec<usize>,
}
impl MeshEdgeLoop {
    pub fn is_closed(&self) -> bool {
        self.vertices.first() == self.vertices.last() && !self.edges.is_empty()
    }
}
impl TriangleMesh {
    /// Grow an edge loop from one topology edge. Boundaries continue through
    /// valence-three vertices and interior loops through valence-four vertices.
    /// Ambiguous or nonmanifold continuations stop; no model tolerance is used.
    pub fn try_edge_loop(&self, seed: usize) -> Result<MeshEdgeLoop, GeometryError> {
        let data = self.topology_data();
        let edges = data.edges.iter().collect::<Vec<_>>();
        let Some(&(&(a, b), _)) = edges.get(seed) else {
            return Err(GeometryError::Degenerate {
                context: "mesh loop seed outside topology",
            });
        };
        let mut adjacent = vec![Vec::new(); data.topological_vertex_count];
        for (i, &(&(a, b), _)) in edges.iter().enumerate() {
            adjacent[a].push(i);
            adjacent[b].push(i);
        }
        let mut seen = BTreeSet::from([seed]);
        let mut extend = |mut edge: usize, mut vertex: usize| {
            let mut path = Vec::new();
            loop {
                let current = edges[edge].1;
                if !(current.count == 1 || current.count == 2)
                    || adjacent[vertex].len() != current.count + 2
                {
                    break;
                }
                let mut candidates = adjacent[vertex].iter().copied().filter(|&other| {
                    if other == edge {
                        return false;
                    }
                    let next = edges[other].1;
                    next.count == current.count
                        && (current.count == 1
                            || !current
                                .uses()
                                .any(|a| next.uses().any(|b| a.face == b.face)))
                });
                let Some(next) = candidates.next() else { break };
                if candidates.next().is_some() || !seen.insert(next) {
                    break;
                }
                let &(a, b) = edges[next].0;
                let end = if vertex == a { b } else { a };
                path.push((next, end));
                edge = next;
                vertex = end;
            }
            path
        };
        let after = extend(seed, b);
        let before = extend(seed, a);
        let mut result = MeshEdgeLoop {
            edges: Vec::new(),
            vertices: Vec::new(),
        };
        for &(edge, vertex) in before.iter().rev() {
            result.vertices.push(vertex);
            result.edges.push(edge);
        }
        result.vertices.push(a);
        result.edges.push(seed);
        result.vertices.push(b);
        for (edge, vertex) in after {
            result.edges.push(edge);
            result.vertices.push(vertex);
        }
        Ok(result)
    }
}
