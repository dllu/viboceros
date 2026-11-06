//! Pickable planar cells and shared-halfedge cancellation for region unions.
use super::*;
mod output;

/// A bounded cell or a union of neighboring cells. Boundaries retain the input
/// rational curves; the first loop is outer and subsequent loops are holes.
#[derive(Clone, Debug)]
pub struct PlanarCurveRegion {
    pub boundaries: Vec<NurbsCurve>,
    pub brep: Brep,
}

/// Reusable construction-plane arrangement. Picking and changing the selected
/// cells do not repeat curve intersection work or change the source curves.
pub struct PlanarCurveArrangement {
    plane: Frame3,
    tolerance: Tolerance,
    surface: Option<NurbsSurface>,
    bounds: [[Real; 2]; 2],
    graph: Option<SurfaceCutGraph>,
    cells: Vec<PlanarCurveRegion>,
    active_edges: Vec<bool>,
}

impl PlanarCurveArrangement {
    pub fn try_new(
        paths: &[NurbsCurve],
        plane: Frame3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let prepared = planar::prepare(paths, plane, tolerance, graph::Mode::Regions)?;
        let Some(planar::Prepared {
            surface,
            bounds,
            graph,
        }) = prepared
        else {
            return Ok(Self {
                plane,
                tolerance,
                surface: None,
                bounds: [[0.; 2]; 2],
                graph: None,
                cells: Vec::new(),
                active_edges: Vec::new(),
            });
        };
        let mut active_edges = vec![false; graph.edges.len()];
        for r in &graph.regions {
            for cycle in std::iter::once(&r.outer).chain(&r.holes) {
                for h in cycle {
                    active_edges[h / 2] = true;
                }
            }
        }
        let cells = graph
            .regions
            .iter()
            .map(|region| output::region(&surface, &graph, region, tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            plane,
            tolerance,
            surface: Some(surface),
            bounds,
            graph: Some(graph),
            cells,
            active_edges,
        })
    }

    pub fn cells(&self) -> &[PlanarCurveRegion] {
        &self.cells
    }

    /// Locate a region using a world-space point projected into the CPlane.
    /// A point in the unbounded exterior returns None.
    pub fn cell_at(&self, point: Point3) -> Result<Option<usize>, GeometryError> {
        let [u, v] = self.plane.projected_coordinates_of(point)?;
        let projected = self.plane.point_at([u, v, 0.])?;
        if let Some(graph) = &self.graph {
            for (i, edge) in graph.edges.iter().enumerate() {
                if !self.active_edges[i] {
                    continue;
                }
                if matches!(edge.kind, SurfaceCutArrangementEdgeKind::Boundary(_)) {
                    continue;
                }
                let hull = edge.spatial.control_point_bounds();
                let epsilon = self.tolerance.absolute();
                if projected
                    .to_array()
                    .into_iter()
                    .zip(hull.min().to_array())
                    .zip(hull.max().to_array())
                    .any(|((x, lo), hi)| x < lo - epsilon || x > hi + epsilon)
                {
                    continue;
                }
                let t = edge.spatial.closest_parameter(projected, self.tolerance)?;
                if projected.distance_to(edge.spatial.evaluate(t)?)? <= epsilon {
                    return Ok(None);
                }
            }
        }
        for (i, cell) in self.cells.iter().enumerate() {
            if cell.brep.faces()[0].contains_parameters(u, v, self.tolerance)? {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    /// Cancel internal shared edges in linear graph work, then reconstruct
    /// outer loops and holes. Point contacts keep separate connected regions.
    pub fn selected(
        &self,
        selected: &[bool],
        combine: bool,
    ) -> Result<Vec<PlanarCurveRegion>, GeometryError> {
        if selected.len() != self.cells.len() {
            return invalid("planar cell selection has the wrong length");
        }
        if !combine || selected.iter().filter(|yes| **yes).count() <= 1 {
            return Ok(self
                .cells
                .iter()
                .zip(selected)
                .filter(|(_, yes)| **yes)
                .map(|(cell, _)| cell.clone())
                .collect());
        }
        let Some(graph) = &self.graph else {
            return Ok(Vec::new());
        };
        let mut next = vec![usize::MAX; graph.edges.len() * 2];
        for (region, &yes) in graph.regions.iter().zip(selected) {
            if !yes {
                continue;
            }
            for cycle in std::iter::once(&region.outer).chain(&region.holes) {
                for (&h, &n) in cycle.iter().zip(cycle.iter().cycle().skip(1)) {
                    next[h] = n;
                }
            }
        }
        let boundary = (0..next.len())
            .map(|h| next[h] != usize::MAX && next[h ^ 1] == usize::MAX)
            .collect::<Vec<_>>();
        let mut visited = vec![false; next.len()];
        let mut outer = Vec::new();
        let mut holes = Vec::new();
        let mut work = 0usize;
        for start in 0..next.len() {
            if !boundary[start] || visited[start] {
                continue;
            }
            let mut cycle = Vec::new();
            let mut h = start;
            loop {
                if visited[h] {
                    if h != start {
                        return invalid("planar union boundary crossed itself");
                    }
                    break;
                }
                visited[h] = true;
                cycle.push(h);
                h = next[h];
                while !boundary[h] {
                    h = next[h ^ 1];
                    work += 1;
                    if work > next.len() {
                        return invalid("planar union boundary did not close");
                    }
                }
            }
            for cycle in simple_cycles(&graph.edges, cycle) {
                if let Some(area) = sampled_surface_cut_cycle_signed_area(&graph.edges, &cycle)? {
                    if area > 0. {
                        outer.push(cycle);
                    } else {
                        holes.push(cycle);
                    }
                }
            }
        }
        let mut regions = outer
            .iter()
            .map(|c| SurfaceCutArrangementRegion {
                outer: c.clone(),
                holes: Vec::new(),
            })
            .collect::<Vec<_>>();
        let epsilon = self
            .bounds
            .map(|r| trim_parameter_epsilon(r, self.tolerance));
        for hole in holes {
            let parent =
                surface_cut_hole_parent(&graph.edges, &outer, &hole, self.bounds, epsilon)?
                    .ok_or(GeometryError::InvalidPlanarFaceBoundary)?;
            regions[parent].holes.push(hole);
        }
        regions
            .iter()
            .map(|region| {
                output::region(
                    self.surface.as_ref().unwrap(),
                    graph,
                    region,
                    self.tolerance,
                )
            })
            .collect()
    }
}

/// Remove immediate out-and-back excursions without removing a bridge which
/// reaches a complete inner boundary before returning to its attachment.
pub(super) fn reduce_spikes(cycle: Vec<usize>) -> Vec<usize> {
    let mut stack = Vec::new();
    for h in cycle {
        if stack.last() == Some(&(h ^ 1)) {
            stack.pop();
        } else {
            stack.push(h);
        }
    }
    let mut start = 0;
    let mut end = stack.len();
    while end > start + 1 && stack[start] == (stack[end - 1] ^ 1) {
        start += 1;
        end -= 1;
    }
    stack[start..end].to_vec()
}

/// Split repeated-node walks into simple cycles. This discards zero-area
/// out-and-back branches and separates holes attached by a bridge.
pub(super) fn simple_cycles(
    edges: &[SurfaceCutArrangementEdge],
    cycle: Vec<usize>,
) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut stack = Vec::<usize>::new();
    let mut positions = BTreeMap::new();
    positions.insert(
        surface_cut_halfedge_nodes(&edges[cycle[0] / 2], cycle[0])[0],
        0usize,
    );
    for h in cycle {
        stack.push(h);
        let end = surface_cut_halfedge_nodes(&edges[h / 2], h)[1];
        if let Some(&i) = positions.get(&end) {
            result.push(stack[i..].to_vec());
            for &edge in &stack[i..] {
                positions.remove(&surface_cut_halfedge_nodes(&edges[edge / 2], edge)[1]);
            }
            stack.truncate(i);
            positions.insert(end, i);
        } else {
            positions.insert(end, stack.len());
        }
    }
    result
}

#[cfg(test)]
mod tests;
