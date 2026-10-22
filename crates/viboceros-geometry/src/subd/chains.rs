//! Source-oriented control chains partitioned by the public native sorter.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubDDirectedEdge {
    pub index: usize,
    pub reversed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubDEdgeChain {
    pub edges: Vec<SubDDirectedEdge>,
    /// Source control-vertex ordinals; a closed chain repeats its first vertex.
    pub vertices: Vec<usize>,
}
impl SubDEdgeChain {
    pub fn is_closed(&self) -> bool {
        !self.edges.is_empty() && self.vertices.first() == self.vertices.last()
    }
}
impl SubD {
    /// Partition unique control-edge ordinals into directed maximal chains.
    /// Branches split chains. Input order does not affect native sorting, and
    /// component marking happens only on a private decoded native copy.
    pub fn try_edge_chains(&self, selected: &[usize]) -> Result<Vec<SubDEdgeChain>, GeometryError> {
        let chains = viboceros_opennurbs::subd_edge_chains(self.archive(), selected)?;
        chains
            .into_iter()
            .map(|chain| {
                let mut vertices = Vec::with_capacity(chain.len() + 1);
                let mut edges = Vec::with_capacity(chain.len());
                for (index, reversed) in chain {
                    let pair = self.edges().get(index).ok_or_else(|| {
                        viboceros_opennurbs::SubDError("Native chain edge outside source".into())
                    })?;
                    let (a, b) = if reversed {
                        (pair[1] as usize, pair[0] as usize)
                    } else {
                        (pair[0] as usize, pair[1] as usize)
                    };
                    if let Some(&last) = vertices.last() {
                        if last != a {
                            return Err(viboceros_opennurbs::SubDError(
                                "Discontinuous native chain".into(),
                            )
                            .into());
                        }
                    } else {
                        vertices.push(a);
                    }
                    vertices.push(b);
                    edges.push(SubDDirectedEdge { index, reversed });
                }
                Ok(SubDEdgeChain { edges, vertices })
            })
            .collect()
    }
}
