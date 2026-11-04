use super::*;
impl SubD {
    /// Cross opposite sides of control quads without using display tessellation.
    pub fn try_edge_ring(&self, seed: usize) -> Result<crate::PolygonEdgeRing, GeometryError> {
        self.edge_ring_index().try_ring(seed)
    }
    pub fn edge_ring_index(&self) -> crate::PolygonEdgeRingIndex {
        let edges = self
            .edges()
            .iter()
            .map(|e| e.map(|i| i as usize))
            .collect::<Vec<_>>();
        let faces = self
            .faces()
            .iter()
            .map(|f| f.iter().map(|&i| i as usize).collect());
        crate::PolygonEdgeRingIndex::new(&edges, faces)
    }
}
