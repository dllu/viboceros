//! Bind graph intervals to shared B-rep restrictions, preserving source clocks.
use super::*;

pub(super) struct Bindings {
    pub vertices: Vec<Option<usize>>,
    fragments: Vec<Vec<usize>>,
    intervals: Vec<Vec<Real>>,
    ranges: Vec<Vec<[Real; 2]>>,
    epsilon: Vec<Real>,
}

impl Bindings {
    pub fn build(
        brep: &mut Brep,
        graph: &SurfaceCutGraph,
        sources: &[Option<input::Source>],
        tolerance: Tolerance,
        work: &mut Work,
    ) -> Result<Self, GeometryError> {
        let mut splits = BTreeMap::<usize, Vec<Real>>::new();
        let mut intervals = vec![Vec::new(); sources.len()];
        let mut epsilon = vec![0.; sources.len()];
        for (i, source) in sources
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.map(|s| (i, s)))
        {
            let curve = &brep.edges[source.edge].curve;
            let domain = curve.domain();
            let eps = surface_cut_curve_parameter_epsilon(curve);
            epsilon[i] = eps;
            let mut breaks = graph.source_breaks[i]
                .iter()
                .map(|p| p.parameter)
                .collect::<Vec<_>>();
            breaks.sort_by(Real::total_cmp);
            breaks.dedup_by(|a, b| (*a - *b).abs() <= eps);
            intervals[i] = breaks;
            let parameters = splits.entry(source.edge).or_default();
            for point in &graph.source_breaks[i] {
                let t = if source.reversed {
                    -point.parameter
                } else {
                    point.parameter
                };
                if t > *domain.start() + eps && t < *domain.end() - eps {
                    parameters.push(t);
                }
            }
            parameters.sort_by(Real::total_cmp);
            parameters.dedup_by(|a, b| (*a - *b).abs() <= eps);
        }
        let splits = splits
            .into_iter()
            .filter(|(_, p)| !p.is_empty())
            .collect::<Vec<_>>();
        work.source_edges.reserve_edges(brep.edges.len());
        let records =
            brep.split_edges_in_place(&splits, work.source_edges.incidence(), tolerance)?;
        work.source_edges.inherit(&records, brep.edges.len())?;
        work.network_curves.inherit(&records, brep.edges.len());
        let records = records.into_iter().collect::<BTreeMap<_, _>>();
        let mut fragments = vec![Vec::new(); sources.len()];
        let mut ranges = vec![Vec::new(); sources.len()];
        let mut vertices = vec![None; graph.nodes.len()];
        for (i, source) in sources
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.map(|s| (i, s)))
        {
            let pieces = records
                .get(&source.edge)
                .cloned()
                .unwrap_or_else(|| vec![source.edge]);
            let mut ends = Vec::with_capacity(pieces.len() + 1);
            for (j, &edge) in pieces.iter().enumerate() {
                let edge = &brep.edges[edge];
                let domain = edge.curve.domain();
                if j == 0 {
                    ends.push((*domain.start(), edge.vertices[0]));
                }
                ends.push((*domain.end(), edge.vertices[1]));
                ranges[i].push([*domain.start(), *domain.end()]);
            }
            fragments[i] = pieces;
            for point in &graph.source_breaks[i] {
                let t = if source.reversed {
                    -point.parameter
                } else {
                    point.parameter
                };
                let k = ends.partition_point(|&(u, _)| u < t);
                let vertex = [k.checked_sub(1), Some(k)]
                    .into_iter()
                    .flatten()
                    .filter_map(|j| ends.get(j))
                    .filter(|&&(u, _)| (u - t).abs() <= epsilon[i])
                    .min_by(|a, b| (a.0 - t).abs().total_cmp(&(b.0 - t).abs()))
                    .map(|&(_, v)| v)
                    .ok_or_else(|| invalid("inset source split has no matching vertex"))?;
                if vertices[point.node].is_some_and(|v| v != vertex) {
                    return Err(invalid("inset source junction maps to different vertices"));
                }
                vertices[point.node] = Some(vertex);
            }
        }
        Ok(Self {
            vertices,
            fragments,
            intervals,
            ranges,
            epsilon,
        })
    }

    pub fn fragment(
        &self,
        index: usize,
        segment: usize,
        reversed: bool,
    ) -> Result<usize, GeometryError> {
        let ts = &self.intervals[index];
        let mut interval = [ts[segment], ts[segment + 1]];
        if reversed {
            interval = [-interval[1], -interval[0]];
        }
        let ranges = &self.ranges[index];
        let k = ranges
            .partition_point(|r| r[0] <= interval[0])
            .saturating_sub(1);
        [k.checked_sub(1), Some(k), k.checked_add(1)]
            .into_iter()
            .flatten()
            .find(|&j| {
                ranges.get(j).is_some_and(|r| {
                    (r[0] - interval[0]).abs() <= self.epsilon[index]
                        && (r[1] - interval[1]).abs() <= self.epsilon[index]
                })
            })
            .map(|j| self.fragments[index][j])
            .ok_or_else(|| invalid("inset source fragment has no matching edge"))
    }
}
