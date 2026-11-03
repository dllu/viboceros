//! Linear-time face ownership and cap-side labels on the common curve graph.
use super::*;

pub(super) fn owners(
    graph: &SurfaceCutGraph,
    left: &[Option<usize>],
    sources: &[Option<input::Source>],
) -> Result<Vec<Option<usize>>, GeometryError> {
    flood(graph, left, |edge| {
        let mut contributors = surface_cut_arrangement_edge_contributors(edge)
            .filter_map(|c| sources[c.source].map(|s| (c, s)));
        let Some((c, source)) = contributors.next() else {
            return Ok(None);
        };
        if contributors.any(|(_, other)| other.edge != source.edge) {
            return Err(invalid(
                "inset networks contain coincident distinct source edges",
            ));
        }
        let mut sides = source.sides;
        if !c.same_direction {
            sides.swap(0, 1);
        }
        Ok(Some(sides))
    })
}

pub(super) fn cap(
    graph: &SurfaceCutGraph,
    left: &[Option<usize>],
    sources: std::ops::Range<usize>,
) -> Result<Vec<bool>, GeometryError> {
    flood(graph, left, |edge| {
        Ok(surface_cut_arrangement_edge_contributors(edge)
            .find(|c| sources.contains(&c.source))
            .map(|c| [c.same_direction, !c.same_direction]))
    })
}

fn flood<T: Copy + Eq>(
    graph: &SurfaceCutGraph,
    left: &[Option<usize>],
    boundary: impl Fn(&SurfaceCutArrangementEdge) -> Result<Option<[T; 2]>, GeometryError>,
) -> Result<Vec<T>, GeometryError> {
    let mut states = vec![None; graph.regions.len()];
    let mut adjacency = vec![Vec::new(); graph.regions.len()];
    let mut queue = VecDeque::new();
    for (i, edge) in graph.edges.iter().enumerate() {
        if let Some(sides) = boundary(edge)? {
            for side in 0..2 {
                if let Some(cell) = left[2 * i + side] {
                    assign(&mut states, &mut queue, cell, sides[side])?;
                }
            }
        } else if let [Some(a), Some(b)] = [left[2 * i], left[2 * i + 1]] {
            adjacency[a].push(b);
            adjacency[b].push(a);
        }
    }
    while let Some(a) = queue.pop_front() {
        let value = states[a].unwrap();
        for &b in &adjacency[a] {
            assign(&mut states, &mut queue, b, value)?;
        }
    }
    states
        .into_iter()
        .map(|s| s.ok_or_else(|| invalid("inset network material classification is unresolved")))
        .collect()
}

fn assign<T: Copy + Eq>(
    states: &mut [Option<T>],
    queue: &mut VecDeque<usize>,
    index: usize,
    value: T,
) -> Result<(), GeometryError> {
    if let Some(previous) = states[index] {
        if previous != value {
            return Err(invalid("inset network material sides conflict"));
        }
    } else {
        states[index] = Some(value);
        queue.push_back(index);
    }
    Ok(())
}
