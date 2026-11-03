use super::*;
#[derive(Clone, Copy)]
pub(super) struct Contact {
    pub vertex: usize,
    pub other: usize,
    pub other_vertex: usize,
}
pub(super) struct Contacts {
    pub rows: Vec<Vec<Contact>>,
    pub neighbors: Vec<BTreeSet<usize>>,
    costs: Vec<usize>,
    work: std::cell::Cell<usize>,
}
impl Contacts {
    pub fn new(sources: &[&SubD], tolerance: f64) -> Result<Self, GeometryError> {
        if !tolerance.is_finite() || tolerance < 0. || sources.len() > 100_000 {
            return Err(error("Invalid SubD join tolerance or input count"));
        }
        let mut count = 0usize;
        let mut faces = 0usize;
        let mut corners = 0usize;
        let mut points = Vec::new();
        let mut costs = Vec::new();
        for (source, s) in sources.iter().enumerate() {
            count = count.saturating_add(s.control_points().len());
            faces = faces.saturating_add(s.faces().len());
            let source_corners = s.faces().iter().map(Vec::len).sum::<usize>();
            corners = corners.saturating_add(source_corners);
            costs.push(
                s.control_points()
                    .len()
                    .saturating_mul(2)
                    .saturating_add(source_corners),
            );
            if count > 100_000 || faces > 100_000 || corners > 400_000 {
                return Err(error("SubD join component limit exceeded"));
            }
            let boundary = s
                .edges()
                .iter()
                .zip(s.edge_face_counts())
                .filter(|(_, n)| **n == 1)
                .flat_map(|(edge, _)| *edge)
                .collect::<BTreeSet<_>>();
            for vertex in boundary {
                points.push((source, vertex as usize, s.control_points()[vertex as usize]));
            }
        }
        let mut rows = vec![Vec::new(); sources.len()];
        let mut neighbors = vec![BTreeSet::new(); sources.len()];
        let spans = std::array::from_fn::<_, 3, _>(|axis| {
            let low = points
                .iter()
                .map(|p| p.2.to_array()[axis])
                .fold(f64::INFINITY, f64::min);
            let high = points
                .iter()
                .map(|p| p.2.to_array()[axis])
                .fold(f64::NEG_INFINITY, f64::max);
            high - low
        });
        let axis = (0..3)
            .max_by(|&a, &b| spans[a].total_cmp(&spans[b]))
            .unwrap();
        points.sort_by(|a, b| {
            a.2.to_array()[axis]
                .total_cmp(&b.2.to_array()[axis])
                .then_with(|| (a.0, a.1).cmp(&(b.0, b.1)))
        });
        let mut work = 0usize;
        let mut pairs = 0usize;
        for (i, &(a, av, p)) in points.iter().enumerate() {
            for &(b, bv, q) in &points[i + 1..] {
                if q.to_array()[axis] - p.to_array()[axis] > tolerance {
                    break;
                }
                work += 1;
                if work > 16_000_000 {
                    return Err(error("SubD join contact search limit exceeded"));
                }
                if a == b || p.distance_to(q)? > tolerance {
                    continue;
                }
                pairs += 1;
                if pairs > 1_000_000 {
                    return Err(error("SubD join contact limit exceeded"));
                }
                rows[a].push(Contact {
                    vertex: av,
                    other: b,
                    other_vertex: bv,
                });
                rows[b].push(Contact {
                    vertex: bv,
                    other: a,
                    other_vertex: av,
                });
                neighbors[a].insert(b);
                neighbors[b].insert(a);
            }
        }
        Ok(Self {
            rows,
            neighbors,
            costs,
            work: std::cell::Cell::new(work),
        })
    }
    pub fn charge_admission(&self, source: usize) -> Result<(), GeometryError> {
        let total = self
            .work
            .get()
            .saturating_add(self.rows[source].len())
            .saturating_add(self.costs[source]);
        if total > 16_000_000 {
            return Err(error("SubD join admission work limit exceeded"));
        }
        self.work.set(total);
        Ok(())
    }
}
