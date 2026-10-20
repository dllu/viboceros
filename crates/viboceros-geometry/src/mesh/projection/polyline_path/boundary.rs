//! Bounded section and naked-boundary routing, retaining original mesh topology.
use super::*;
use std::{cmp::Ordering, collections::BinaryHeap};
const MAX_NODES: usize = 16_384;
const MAX_WORK: usize = 4_000_000;
#[derive(Debug)]
pub(in crate::mesh::projection) struct Boundaries {
    loops: Vec<(usize, Polyline3)>,
}
impl Boundaries {
    pub(super) fn new(mesh: &TriangleMesh) -> Result<Self, GeometryError> {
        let mut loops = Vec::new();
        let mut count = 0;
        for (component, piece) in mesh.try_disjoint_pieces(1024)?.into_iter().enumerate() {
            for boundary in piece.boundary_polylines(Tolerance::NUMERICAL_VALIDATION)? {
                if !closed_section(&boundary) {
                    return Err(GeometryError::MeshPolylinePathUnresolved);
                }
                count += boundary.segment_count();
                if count > MAX_NODES {
                    return Err(GeometryError::MeshPullLimit);
                }
                loops.push((component, boundary));
            }
        }
        Ok(Self { loops })
    }
}
struct Graph {
    points: Vec<Point3>,
    keys: BTreeMap<[u64; 3], usize>,
    edges: Vec<Vec<(usize, Real)>>,
    corners: Vec<bool>,
    work: usize,
}
impl Graph {
    fn new() -> Self {
        Self {
            points: Vec::new(),
            keys: BTreeMap::new(),
            edges: Vec::new(),
            corners: Vec::new(),
            work: 0,
        }
    }
    fn charge(&mut self, n: usize) -> Result<(), GeometryError> {
        self.work = self
            .work
            .checked_add(n)
            .filter(|n| *n <= MAX_WORK)
            .ok_or(GeometryError::MeshPullLimit)?;
        Ok(())
    }
    fn node(&mut self, point: Point3) -> Result<usize, GeometryError> {
        let key = point
            .to_array()
            .map(super::super::super::canonical_coordinate_bits);
        if let Some(id) = self.keys.get(&key) {
            return Ok(*id);
        }
        if self.points.len() == MAX_NODES {
            return Err(GeometryError::MeshPullLimit);
        }
        let id = self.points.len();
        self.keys.insert(key, id);
        self.points.push(point);
        self.edges.push(Vec::new());
        self.corners.push(false);
        Ok(id)
    }
    fn edge(&mut self, a: usize, b: usize) -> Result<(), GeometryError> {
        if a == b {
            return Ok(());
        }
        self.charge(1)?;
        let length = self.points[a].distance_to(self.points[b])?;
        self.edges[a].push((b, length));
        self.edges[b].push((a, length));
        Ok(())
    }
    fn chain(
        &mut self,
        curve: &Polyline3,
        mut cuts: Vec<(Real, usize)>,
    ) -> Result<(), GeometryError> {
        for (i, p) in curve.vertices().iter().enumerate() {
            let id = self.node(*p)?;
            cuts.push((i as Real, id));
        }
        cuts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for pair in cuts.windows(2) {
            self.edge(pair[0].1, pair[1].1)?;
        }
        Ok(())
    }
    fn shortest(&mut self, start: usize, end: usize) -> Result<Polyline3, GeometryError> {
        let mut distance = vec![Real::INFINITY; self.points.len()];
        let mut previous = vec![usize::MAX; self.points.len()];
        distance[start] = 0.;
        let mut branch = vec![None; self.points.len()];
        if self.corners[start] {
            branch[start] = Some((0, start));
        }
        let mut heap = BinaryHeap::from([Visit {
            distance: 0.,
            node: start,
            branch: branch[start],
        }]);
        while let Some(Visit {
            distance: cost,
            node,
            branch: seed,
        }) = heap.pop()
        {
            if cost > distance[node] || seed != branch[node] {
                continue;
            }
            if node == end {
                break;
            }
            self.charge(self.edges[node].len())?;
            for &(next, length) in &self.edges[node] {
                let candidate = cost + length;
                if !candidate.is_finite() {
                    return Err(GeometryError::MeshPullLimit);
                }
                let next_branch = seed
                    .or_else(|| self.corners[next].then_some((rounded_distance(candidate), next)));
                let old = distance[next];
                let roundoff = 16. * Real::EPSILON * old.abs().max(candidate.abs());
                let shorter = !old.is_finite() || candidate < old && old - candidate > roundoff;
                let tied = (old - candidate).abs() <= roundoff;
                // Keep the destination's first boundary wavefront branch
                // through later equal-distance corners, rather than replacing
                // it with a newly encountered vertex-index tie.
                if shorter || tied && next_branch < branch[next] {
                    distance[next] = candidate;
                    branch[next] = next_branch;
                    previous[next] = node;
                    heap.push(Visit {
                        distance: candidate,
                        node: next,
                        branch: next_branch,
                    });
                }
            }
        }
        if !distance[end].is_finite() {
            return Err(GeometryError::MeshPolylinePathUnresolved);
        }
        let mut nodes = vec![end];
        let mut node = end;
        while node != start {
            node = previous[node];
            if node == usize::MAX || nodes.len() > MAX_NODES {
                return Err(GeometryError::MeshPolylinePathUnresolved);
            }
            nodes.push(node);
        }
        nodes.reverse();
        compact(nodes.into_iter().map(|i| self.points[i]).collect())
    }
}
fn rounded_distance(distance: Real) -> u64 {
    // All heap distances are finite and nonnegative. Removing four mantissa
    // bits keeps the heap order transitive while suppressing last-bit noise
    // between geometrically equal detours. This is independent of model epsilon.
    distance.to_bits().saturating_add(8) >> 4
}
struct Visit {
    distance: Real,
    node: usize,
    branch: Option<(u64, usize)>,
}
impl Visit {
    fn priority(&self) -> (u64, Option<(u64, usize)>, usize) {
        (rounded_distance(self.distance), self.branch, self.node)
    }
}
impl PartialEq for Visit {
    fn eq(&self, other: &Self) -> bool {
        self.priority() == other.priority()
    }
}
impl Eq for Visit {}
impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Visit {
    fn cmp(&self, other: &Self) -> Ordering {
        other.priority().cmp(&self.priority())
    }
}

pub(super) fn route(
    index: &MeshProjectionIndex<'_>,
    sections: &[Polyline3],
    start: Point3,
    end: Point3,
    tolerance: Tolerance,
) -> Result<Polyline3, GeometryError> {
    let boundaries = index
        .boundary_routes
        .get_or_init(|| Boundaries::new(&index.mesh))
        .as_ref()
        .map_err(Clone::clone)?;
    let mut graph = Graph::new();
    let source = graph.node(start)?;
    let target = graph.node(end)?;
    let mut endpoints = Vec::new();
    for section in sections {
        let mut cuts = Vec::new();
        for (point, id) in [(start, source), (end, target)] {
            graph.charge(section.segment_count())?;
            if let Some(t) = station(section, point, tolerance)? {
                cuts.push((t, id));
            }
        }
        graph.chain(section, cuts)?;
        if !closed_section(section) {
            for p in [section.vertices()[0], *section.vertices().last().unwrap()] {
                let id = graph.node(p)?;
                endpoints.push(id);
            }
        }
    }
    endpoints.sort_unstable();
    endpoints.dedup();
    let mut boundary_cuts = vec![Vec::new(); boundaries.loops.len()];
    for endpoint in endpoints {
        let mut locations = Vec::new();
        for (i, (_, boundary)) in boundaries.loops.iter().enumerate() {
            graph.charge(boundary.segment_count())?;
            if let Some(t) = station(boundary, graph.points[endpoint], tolerance)? {
                locations.push((i, t));
            }
        }
        if locations.len() > 1 {
            return Err(GeometryError::MeshPolylinePathUnresolved);
        }
        if let Some((i, t)) = locations.pop() {
            boundary_cuts[i].push((t, endpoint));
        }
    }
    let mut corners = BTreeMap::<usize, Vec<usize>>::new();
    for ((component, boundary), cuts) in boundaries.loops.iter().zip(boundary_cuts) {
        graph.chain(boundary, cuts)?;
        for point in &boundary.vertices()[..boundary.segment_count()] {
            let id = graph.node(*point)?;
            graph.corners[id] = true;
            corners.entry(*component).or_default().push(id);
        }
    }
    let groups = corners.into_values().collect::<Vec<_>>();
    for (i, group) in groups.iter().enumerate() {
        for other in &groups[i + 1..] {
            graph.charge(
                group
                    .len()
                    .checked_mul(other.len())
                    .ok_or(GeometryError::MeshPullLimit)?,
            )?;
            for &a in group {
                for &b in other {
                    graph.edge(a, b)?;
                }
            }
        }
    }
    graph.shortest(target, source).map(|path| path.reversed())
}
