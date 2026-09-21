use super::*;

#[derive(Clone, Copy)]
struct Entry {
    edge: usize,
    low: [Real; 3],
    high: [Real; 3],
}
#[derive(Clone, Copy)]
struct Node {
    low: [Real; 3],
    high: [Real; 3],
    range: [usize; 2],
    children: Option<[usize; 2]>,
}

pub(super) fn find(
    brep: &Brep,
    distance: Real,
    budget: &mut Budget,
) -> Result<Vec<(usize, usize)>, GeometryError> {
    let counts = brep.edge_use_counts();
    let count = counts.iter().filter(|&&n| n == 1).count();
    if count > MAX_NAKED {
        return Err(invalid("too many naked B-rep join edges"));
    }
    let mut entries = Vec::with_capacity(count);
    for (edge, &uses) in counts.iter().enumerate() {
        if uses != 1 {
            continue;
        }
        budget.charge(brep.edges[edge].curve.control_points().len())?;
        let bounds = brep.edges[edge].curve.control_point_bounds();
        entries.push(Entry {
            edge,
            low: bounds.min().to_array(),
            high: bounds.max().to_array(),
        });
    }
    if entries.len() < 2 {
        return Ok(Vec::new());
    }
    let mut nodes = Vec::new();
    build(&mut entries, 0, &mut nodes);
    let mut pairs = Vec::new();
    visit(0, 0, &nodes, &entries, distance, budget, &mut pairs)?;
    pairs.sort_unstable();
    Ok(pairs)
}

fn separated(
    a_low: [Real; 3],
    a_high: [Real; 3],
    b_low: [Real; 3],
    b_high: [Real; 3],
    r: Real,
) -> bool {
    // Monotonic rounded subtraction cannot exclude an exact gap <= r.
    (0..3).any(|i| a_low[i] - b_high[i] > r || b_low[i] - a_high[i] > r)
}

fn build(entries: &mut [Entry], start: usize, nodes: &mut Vec<Node>) -> usize {
    let mut low = [Real::INFINITY; 3];
    let mut high = [Real::NEG_INFINITY; 3];
    for e in entries.iter() {
        for i in 0..3 {
            low[i] = low[i].min(e.low[i]);
            high[i] = high[i].max(e.high[i]);
        }
    }
    let root = nodes.len();
    nodes.push(Node {
        low,
        high,
        range: [start, start + entries.len()],
        children: None,
    });
    if entries.len() > 8 {
        let axis = (0..3)
            .max_by(|&a, &b| (high[a] - low[a]).total_cmp(&(high[b] - low[b])))
            .unwrap();
        let middle = entries.len() / 2;
        entries.select_nth_unstable_by(middle, |a, b| {
            a.low[axis]
                .midpoint(a.high[axis])
                .total_cmp(&b.low[axis].midpoint(b.high[axis]))
                .then_with(|| a.edge.cmp(&b.edge))
        });
        let (a, b) = entries.split_at_mut(middle);
        nodes[root].children = Some([build(a, start, nodes), build(b, start + middle, nodes)]);
    }
    root
}

fn visit(
    a: usize,
    b: usize,
    nodes: &[Node],
    entries: &[Entry],
    r: Real,
    budget: &mut Budget,
    pairs: &mut Vec<(usize, usize)>,
) -> Result<(), GeometryError> {
    budget.charge(1)?;
    let an = nodes[a];
    let bn = nodes[b];
    if separated(an.low, an.high, bn.low, bn.high, r) {
        return Ok(());
    }
    if a == b {
        if let Some([lo, hi]) = an.children {
            visit(lo, lo, nodes, entries, r, budget, pairs)?;
            visit(lo, hi, nodes, entries, r, budget, pairs)?;
            return visit(hi, hi, nodes, entries, r, budget, pairs);
        }
    } else if let Some([lo, hi]) = an.children {
        visit(lo, b, nodes, entries, r, budget, pairs)?;
        return visit(hi, b, nodes, entries, r, budget, pairs);
    } else if let Some([lo, hi]) = bn.children {
        visit(a, lo, nodes, entries, r, budget, pairs)?;
        return visit(a, hi, nodes, entries, r, budget, pairs);
    }
    for i in an.range[0]..an.range[1] {
        let first = if a == b { i + 1 } else { bn.range[0] };
        for j in first..bn.range[1] {
            budget.charge(1)?;
            let (a, b) = (entries[i], entries[j]);
            if separated(a.low, a.high, b.low, b.high, r) {
                continue;
            }
            if pairs.len() == MAX_CANDIDATES {
                return Err(invalid("too many B-rep join candidates"));
            }
            pairs.push((a.edge.min(b.edge), a.edge.max(b.edge)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_search_matches_all_pairs_without_duplicates_across_coordinate_scales() {
        let mut state = 8329_u64;
        for scale in [1e-280, 1., 1e280] {
            let entries = (0..129)
                .map(|edge| {
                    let low = std::array::from_fn(|_| {
                        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                        ((state >> 32) % 16) as Real * scale
                    });
                    Entry {
                        edge,
                        low,
                        high: low.map(|x| x + 2. * scale),
                    }
                })
                .collect::<Vec<_>>();
            for distance in [0., scale, 20. * scale] {
                let mut expected = Vec::new();
                for (i, a) in entries.iter().enumerate() {
                    for b in &entries[i + 1..] {
                        if !separated(a.low, a.high, b.low, b.high, distance) {
                            expected.push((a.edge, b.edge));
                        }
                    }
                }
                let mut indexed = entries.clone();
                let mut nodes = Vec::new();
                build(&mut indexed, 0, &mut nodes);
                let mut actual = Vec::new();
                visit(
                    0,
                    0,
                    &nodes,
                    &indexed,
                    distance,
                    &mut Budget(MAX_WORK),
                    &mut actual,
                )
                .unwrap();
                actual.sort_unstable();
                assert_eq!(actual, expected);
            }
        }
        assert!(!separated(
            [1e15, 0., 0.],
            [1e15, 1., 1.],
            [1e15, 1.001, 0.],
            [1e15, 2., 1.],
            0.002
        ));
        assert!(separated(
            [1e15, 0., 0.],
            [1e15, 1., 1.],
            [1e15, 1.001, 0.],
            [1e15, 2., 1.],
            0.0005
        ));
    }
}
