//! Recover unsplit guides before paths passing through explicit point seeds.
use super::*;
use spade::Intersection;

pub(super) struct Segment {
    pub nodes: [usize; 2],
    pub original: bool,
    pub original_ends: [usize; 2],
}
pub(super) struct Prepared {
    pub order: Vec<usize>,
    pub pairs: Vec<Vec<Segment>>,
}
pub(super) fn prepare(
    cdt: &Cdt,
    remap: &[usize],
    chains: &[(Range<usize>, bool)],
    guide_count: usize,
    point_nodes: &BTreeSet<usize>,
    automatic: bool,
    work: &mut usize,
) -> Result<Prepared, GeometryError> {
    let mut handles = vec![None; cdt.num_vertices()];
    for vertex in cdt.vertices() {
        handles[vertex.data().source] = Some(vertex.fix());
    }
    let mut pairs = Vec::with_capacity(chains.len());
    let mut deferred = vec![false; guide_count];
    for (chain, (range, _)) in chains.iter().enumerate() {
        let mut original = remap[range.clone()]
            .windows(2)
            .map(|p| ([p[0], p[1]], true))
            .collect::<Vec<_>>();
        if chain < guide_count
            && automatic
            && range.len() > 2
            && remap[range.start] != remap[range.end - 1]
        {
            original.push(([remap[range.end - 1], remap[range.start]], false));
        }
        let mut segments = Vec::new();
        for (pair, is_original) in original {
            let mut nodes = vec![pair[0]];
            if automatic && chain < guide_count && !point_nodes.is_empty() && pair[0] != pair[1] {
                let from = cdt.vertex(handles[pair[0]].unwrap()).data().clone();
                let to = cdt.vertex(handles[pair[1]].unwrap()).data().clone();
                let mut candidates = BTreeSet::new();
                for intersection in LineIntersectionIterator::new_from_handles(
                    cdt,
                    handles[pair[0]].unwrap(),
                    handles[pair[1]].unwrap(),
                ) {
                    charge(work, 1)?;
                    match intersection {
                        Intersection::VertexIntersection(vertex) => {
                            let node = vertex.data().source;
                            if !pair.contains(&node) && point_nodes.contains(&node) {
                                candidates.insert(node);
                            }
                        }
                        Intersection::EdgeOverlap(edge) => {
                            for vertex in [edge.from(), edge.to()] {
                                let node = vertex.data().source;
                                if !pair.contains(&node) && point_nodes.contains(&node) {
                                    candidates.insert(node);
                                }
                            }
                        }
                        Intersection::EdgeIntersection(edge) => {
                            for vertex in [edge.from(), edge.to()] {
                                let node = vertex.data().source;
                                if !pair.contains(&node)
                                    && point_nodes.contains(&node)
                                    && !candidates.contains(&node)
                                {
                                    // Exact spatial predicates have a larger
                                    // work charge than chart-edge traversal.
                                    charge(work, 64)?;
                                    if on_segment(&from, &to, vertex.data()) {
                                        candidates.insert(node);
                                    }
                                }
                            }
                        }
                    }
                }
                let direction = [
                    to.position.x - from.position.x,
                    to.position.y - from.position.y,
                ];
                let axis = usize::from(direction[1].abs() > direction[0].abs());
                let coordinate = |node: usize| {
                    let p = cdt.vertex(handles[node].unwrap()).position();
                    if axis == 0 { p.x } else { p.y }
                };
                let mut candidates = candidates.into_iter().collect::<Vec<_>>();
                candidates.sort_by(|a, b| {
                    if direction[axis] > 0. {
                        coordinate(*a).total_cmp(&coordinate(*b))
                    } else {
                        coordinate(*b).total_cmp(&coordinate(*a))
                    }
                });
                deferred[chain] |= !candidates.is_empty();
                nodes.extend(candidates);
            }
            nodes.push(pair[1]);
            segments.extend(nodes.windows(2).map(|p| Segment {
                nodes: [p[0], p[1]],
                original: is_original,
                original_ends: pair,
            }));
        }
        pairs.push(segments);
    }
    let mut order = (0..guide_count).collect::<Vec<_>>();
    order.sort_by_key(|i| deferred[*i]);
    order.extend(guide_count..chains.len());
    Ok(Prepared { order, pairs })
}

fn on_segment(from: &Vertex, to: &Vertex, point: &Vertex) -> bool {
    let a = from.point.to_array();
    let b = to.point.to_array();
    let p = point.point.to_array();
    let Some(axis) = (0..3).find(|&i| a[i] != b[i]) else {
        return false;
    };
    if p[axis] <= a[axis].min(b[axis]) || p[axis] >= a[axis].max(b[axis]) {
        return false;
    }
    // Projection can put an exactly collinear spatial seed one rounding step
    // beside a chart edge. Only such near-line candidates reach the exact test.
    let d = [
        to.position.x - from.position.x,
        to.position.y - from.position.y,
    ];
    let q = [
        point.position.x - from.position.x,
        point.position.y - from.position.y,
    ];
    if d[0].mul_add(q[1], -d[1] * q[0]).abs()
        > 64. * Real::EPSILON * (d[0].abs() + d[1].abs()) * (1. + q[0].abs() + q[1].abs())
    {
        return false;
    }
    let d: [Rational; 3] = std::array::from_fn(|i| rational(b[i]) - rational(a[i]));
    let q: [Rational; 3] = std::array::from_fn(|i| rational(p[i]) - rational(a[i]));
    (0..3).all(|i| &d[axis] * &q[i] == &d[i] * &q[axis])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vertex(point: [Real; 3], chart: [Real; 2]) -> Vertex {
        Vertex {
            point: Point3::try_from(point).unwrap(),
            position: TriangulationPoint2::new(chart[0], chart[1]),
            source: 0,
        }
    }
    #[test]
    fn spatial_witness_accepts_chart_rounding_and_preserves_transverse_features() {
        for scale in [1e-100, 1., 1e100] {
            let a = vertex([0.; 3], [0., 0.]);
            let b = vertex([3. * scale, 0., 0.], [1., 0.]);
            let exact = vertex([scale, 0., 0.], [1. / 3., Real::EPSILON]);
            assert!(on_segment(&a, &b, &exact));
            let transverse = vertex(
                [scale, Real::MIN_POSITIVE * Real::EPSILON, 0.],
                [1. / 3., 0.],
            );
            assert!(!on_segment(&a, &b, &transverse));
            assert!(!on_segment(&a, &b, &a));
            assert!(!on_segment(&a, &b, &b));
        }
    }
}
