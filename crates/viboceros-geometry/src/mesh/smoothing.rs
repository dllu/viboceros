use super::*;
use crate::smoothing::{
    mean, projected_step, projection_matrix, smooth_graph, validate_selection, world_frame,
};
use crate::{Frame3, SmoothingCoordinates, SmoothingOptions};

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
        let (neighbors, movable) = self.smoothing_graph(options, selected)?;
        let vertices = smooth_graph(self.vertices.clone(), &neighbors, &movable, options, frame)?;
        self.try_with_edited_vertices(vertices)
    }

    /// Object coordinates use initial raw-vertex normals for every step.
    pub fn try_smoothed_in(
        &self,
        options: SmoothingOptions,
        coordinates: SmoothingCoordinates,
        selected: Option<&BTreeSet<usize>>,
    ) -> Result<Self, GeometryError> {
        match coordinates {
            SmoothingCoordinates::World => {
                return self.try_smoothed(options, world_frame(), selected);
            }
            SmoothingCoordinates::CPlane(frame) => {
                return self.try_smoothed(options, frame, selected);
            }
            SmoothingCoordinates::Object => {}
        }
        options.validate()?;
        let (neighbors, movable) = self.smoothing_graph(options, selected)?;
        if options.factor == 0.0 || !options.axes.contains(&true) {
            return Ok(self.clone());
        }
        let matrices = self
            .raw_vertex_normals()?
            .into_iter()
            .map(|n| {
                Frame3::try_from_normal(
                    Point3::try_from([0.; 3]).unwrap(),
                    n,
                    Tolerance::MESH_VALIDATION,
                )
                .map(|f| projection_matrix(options.axes, f))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut points = self.vertices.clone();
        let mut next = points.clone();
        for _ in 0..options.steps {
            for (i, &point) in points.iter().enumerate() {
                next[i] = if movable[i] && !neighbors[i].is_empty() {
                    projected_step(
                        point,
                        mean(&points, &neighbors[i])?,
                        options.factor,
                        matrices[i],
                    )?
                } else {
                    point
                };
            }
            if next == points {
                break;
            }
            std::mem::swap(&mut points, &mut next);
        }
        self.try_with_edited_vertices(points)
    }

    fn smoothing_graph(
        &self,
        options: SmoothingOptions,
        selected: Option<&BTreeSet<usize>>,
    ) -> Result<(Vec<Vec<usize>>, Vec<bool>), GeometryError> {
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
        Ok((neighbors, movable))
    }
}
