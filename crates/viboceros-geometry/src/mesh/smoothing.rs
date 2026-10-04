use super::*;
use crate::smoothing::{smooth_graph, validate_selection};
use crate::{Frame3, SmoothingOptions};

impl TriangleMesh {
    /// Synchronously averages adjacent mesh vertices along polygon edges.
    /// Naked edge vertices can be fixed. Selected indices address raw vertices,
    /// allowing unwelded copies at the same position to move independently.
    pub fn try_smoothed(
        &self,
        options: SmoothingOptions,
        frame: Frame3,
        selected: Option<&BTreeSet<usize>>,
    ) -> Result<Self, GeometryError> {
        options.validate()?;
        validate_selection(selected, self.vertices.len())?;
        let topology = self.topology_data();
        let mut representatives = vec![usize::MAX; topology.topological_vertex_count];
        for (raw, &group) in topology.topological_vertices.iter().enumerate() {
            representatives[group] = representatives[group].min(raw);
        }
        let mut adjacent = vec![BTreeSet::new(); topology.topological_vertex_count];
        let mut boundary = vec![false; topology.topological_vertex_count];
        for (&(a, b), edge) in &topology.edges {
            adjacent[a].insert(representatives[b]);
            adjacent[b].insert(representatives[a]);
            if edge.count == 1 {
                boundary[a] = true;
                boundary[b] = true;
            }
        }
        let neighbors = topology
            .topological_vertices
            .iter()
            .map(|&g| adjacent[g].iter().copied().collect())
            .collect::<Vec<Vec<usize>>>();
        let movable = topology
            .topological_vertices
            .iter()
            .enumerate()
            .map(|(i, &g)| {
                selected.is_none_or(|s| s.contains(&i)) && !(options.fix_boundaries && boundary[g])
            })
            .collect::<Vec<_>>();
        let vertices = smooth_graph(self.vertices.clone(), &neighbors, &movable, options, frame)?;
        self.try_with_edited_vertices(vertices)
    }
}
