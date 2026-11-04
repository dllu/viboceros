use super::*;
impl TriangleMesh {
    /// Cross opposite sides of quads in exact-location welded topology.
    /// Triangles, degenerate quads and nonmanifold continuations stop the walk.
    pub fn try_edge_ring(&self, seed: usize) -> Result<crate::PolygonEdgeRing, GeometryError> {
        self.edge_ring_index().try_ring(seed)
    }
    pub fn edge_ring_index(&self) -> crate::PolygonEdgeRingIndex {
        let data = self.topology_data();
        let edges = data.edges.keys().map(|&(a, b)| [a, b]).collect::<Vec<_>>();
        let faces = self.faces.iter().map(|f| {
            f.indices()
                .iter()
                .map(|&i| data.topological_vertices[i as usize])
                .collect()
        });
        crate::PolygonEdgeRingIndex::new(&edges, faces)
    }
}
