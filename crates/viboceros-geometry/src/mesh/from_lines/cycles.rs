//! Bounded chordless-cycle enumeration and sparse binary cycle independence.
use super::*;
pub(super) fn recover(
    count: usize,
    edges: &BTreeSet<[usize; 2]>,
    max_sides: usize,
    work: &mut usize,
) -> Result<Vec<Vec<u32>>, GeometryError> {
    let mut neighbors = vec![BTreeSet::new(); count];
    for &[a, b] in edges {
        neighbors[a].insert(b);
        neighbors[b].insert(a);
    }
    let mut queue = (0..count)
        .filter(|&i| neighbors[i].len() < 2)
        .collect::<VecDeque<_>>();
    while let Some(node) = queue.pop_front() {
        for other in std::mem::take(&mut neighbors[node]) {
            neighbors[other].remove(&node);
            if neighbors[other].len() == 1 {
                queue.push_back(other);
            }
        }
    }
    let graph = neighbors
        .iter()
        .map(|n| n.iter().copied().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    let mut visited = vec![false; count];
    let mut adjacent = vec![0usize; count];
    let mut remaining_degree = graph.iter().map(Vec::len).collect::<Vec<_>>();
    let mut active = remaining_degree
        .iter()
        .map(|&degree| degree >= 2)
        .collect::<Vec<_>>();
    for root in 0..count {
        if !active[root] {
            continue;
        }
        let mut path = vec![root];
        let mut next = vec![0usize];
        visited[root] = true;
        for &other in &graph[root] {
            adjacent[other] += 1;
        }
        while let Some(&node) = path.last() {
            let at = next.last_mut().unwrap();
            if *at == graph[node].len() {
                visited[node] = false;
                for &other in &graph[node] {
                    adjacent[other] -= 1;
                }
                path.pop();
                next.pop();
                continue;
            }
            let other = graph[node][*at];
            *at += 1;
            charge(work, 1)?;
            if other <= root || visited[other] || path.len() >= max_sides || !active[other] {
                continue;
            }
            let closing = neighbors[other].contains(&root);
            if adjacent[other] > 1 + usize::from(closing && path.len() > 1) {
                continue;
            }
            if closing && path.len() >= 2 {
                if path[1] < other {
                    let mut cycle = path.iter().map(|&i| i as u32).collect::<Vec<_>>();
                    cycle.push(other as u32);
                    found.push(cycle);
                    if found.len() > TriangleMesh::MAX_LINE_NETWORK_LINES {
                        return Err(GeometryError::MeshLineNetworkLimit);
                    }
                }
                continue;
            }
            visited[other] = true;
            charge(work, graph[other].len())?;
            for &neighbor in &graph[other] {
                adjacent[neighbor] += 1;
            }
            path.push(other);
            next.push(0);
        }
        // Later canonical roots cannot use this vertex. Peel newly exposed
        // trees so a long ring is traversed once instead of once per vertex.
        let mut peel = VecDeque::from([root]);
        active[root] = false;
        while let Some(node) = peel.pop_front() {
            charge(work, graph[node].len())?;
            for &other in &graph[node] {
                if active[other] {
                    remaining_degree[other] -= 1;
                    if remaining_degree[other] < 2 {
                        active[other] = false;
                        peel.push_back(other);
                    }
                }
            }
        }
    }
    found.sort_by_key(Vec::len);
    let edge_ids = edges
        .iter()
        .enumerate()
        .map(|(i, &e)| (e, i))
        .collect::<BTreeMap<_, _>>();
    let mut basis = BTreeMap::<usize, BTreeSet<usize>>::new();
    let mut result = Vec::new();
    for cycle in found {
        charge(work, cycle.len())?;
        let mut row = cycle
            .iter()
            .zip(cycle.iter().cycle().skip(1))
            .take(cycle.len())
            .map(|(&a, &b)| edge_ids[&[(a as usize).min(b as usize), (a as usize).max(b as usize)]])
            .collect::<BTreeSet<_>>();
        while let Some(&pivot) = row.first() {
            if let Some(previous) = basis.get(&pivot) {
                charge(work, row.len() + previous.len())?;
                row = row.symmetric_difference(previous).copied().collect();
            } else {
                break;
            }
        }
        let independent = !row.is_empty();
        if let Some(&pivot) = row.first() {
            basis.insert(pivot, row);
        }
        if independent || cycle.len() == 3 {
            result.push(cycle);
        }
    }
    Ok(result)
}
