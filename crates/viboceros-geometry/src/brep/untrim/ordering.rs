//! Component ordering compatibility for rectangular exterior restoration.
//!
//! Geometry is constructed independently in `boundary`. The native command's
//! edge allocation is observable through subsequent indexed component picks.
//! Its four-side allocation classes are measured for every source permutation;
//! this small table changes references only, never curves or vertices.
use super::*;

mod rectangular;

pub(super) fn apply(
    source: &Brep,
    marked: &[bool],
    new_edges: [Option<usize>; 4],
    work: &mut Brep,
) {
    let Some(order) = rectangle_order(source, marked, new_edges, work) else {
        return;
    };
    let mut inverse = vec![0; order.len()];
    for (new, &old) in order.iter().enumerate() {
        inverse[old] = new;
    }
    let mut edges = std::mem::take(&mut work.edges)
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
    work.edges = order
        .into_iter()
        .map(|old| edges[old].take().unwrap())
        .collect();
    for face in &mut work.faces {
        for ring in &mut face.loops {
            for trim in &mut ring.trims {
                trim.edge = trim.edge.map(|old| inverse[old]);
            }
        }
    }
}

fn rectangle_order(
    source: &Brep,
    marked: &[bool],
    new_edges: [Option<usize>; 4],
    work: &Brep,
) -> Option<Vec<usize>> {
    let ring = &source.faces[0].loops[0];
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    for trim in &ring.trims {
        // The allocation classes require straight UV sides, including split
        // sides. Cached iso tags and scalar curve intervals are not classifiers.
        if trim.curve.degree() != 1 || trim.curve.control_points().len() != 2 || trim.reversed_3d {
            return None;
        }
        for point in trim.curve.control_points() {
            let coordinates = [point.point().x(), point.point().y()];
            for axis in 0..2 {
                bounds[axis][0] = bounds[axis][0].min(coordinates[axis]);
                bounds[axis][1] = bounds[axis][1].max(coordinates[axis]);
            }
        }
    }
    if bounds.iter().any(|range| range[0] >= range[1]) {
        return None;
    }
    let mut sides = [None; 4];
    let mut removed_side = None;
    for (index, trim) in ring.trims.iter().enumerate() {
        let side = rectangle_side(&trim.curve, bounds)?;
        let edge = trim.edge?;
        if edge < 4 {
            if sides[edge].is_some_and(|previous| previous != side) {
                return None;
            }
            sides[edge] = Some(side);
        }
        if marked[index] {
            if removed_side.is_some_and(|previous| previous != side) {
                return None;
            }
            removed_side = Some(side);
        }
    }
    let sides = sides.map(|side| side.unwrap_or(usize::MAX));
    if (0..4).any(|side| sides.iter().filter(|&&value| value == side).count() != 1) {
        return None;
    }
    let removed_side = removed_side?;
    if (0..4).any(|side| new_edges[side].is_some() != (side != (removed_side + 2) % 4)) {
        return None;
    }
    let mut rank = 0;
    for (index, &side) in sides.iter().enumerate() {
        rank = rank * (4 - index)
            + sides[index + 1..]
                .iter()
                .filter(|&&next| next < side)
                .count();
    }
    let allocation = rectangular::ORDER[removed_side][rank];
    if allocation.iter().any(|&value| value >= 8) {
        return None;
    }
    let mut used = vec![false; work.edges.len()];
    for face in &work.faces {
        for ring in &face.loops {
            for trim in &ring.trims {
                if let Some(edge) = trim.edge {
                    used[edge] = true;
                }
            }
        }
    }
    let symbol = |value: u8| {
        if value < 4 {
            Some(value as usize)
        } else {
            new_edges[value as usize - 4]
        }
    };
    let mut order = Vec::with_capacity(work.edges.len());
    for &value in &allocation[..4] {
        order.push(symbol(value)?);
    }
    // Additional original fragments and hole edges keep their source order.
    order.extend((4..source.edges.len()).filter(|&edge| used[edge]));
    for &value in &allocation[4..] {
        order.push(symbol(value)?);
    }
    let mut seen = vec![false; work.edges.len()];
    for &edge in &order {
        if edge >= used.len() || !used[edge] || seen[edge] {
            return None;
        }
        seen[edge] = true;
    }
    if used.iter().zip(&seen).any(|(&used, &seen)| used && !seen) {
        return None;
    }
    // Complete permutation, including orphaned source edges that compaction
    // will discard. No live incidence or geometry can disappear here.
    order.extend((0..work.edges.len()).filter(|&edge| !seen[edge]));
    Some(order)
}

fn rectangle_side(curve: &NurbsCurve2, bounds: [[Real; 2]; 2]) -> Option<usize> {
    let points = curve.control_points();
    [
        (1, bounds[1][0]),
        (0, bounds[0][1]),
        (1, bounds[1][1]),
        (0, bounds[0][0]),
    ]
    .iter()
    .position(|&(axis, value)| {
        points.iter().all(|point| {
            let p = point.point();
            (if axis == 0 { p.x() } else { p.y() }) == value
        })
    })
}
