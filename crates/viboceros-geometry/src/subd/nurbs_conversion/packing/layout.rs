//! Lay out rectangular components, retaining a cut for each wrapped direction.
use super::*;
use graph::Graph;
use std::collections::VecDeque;

pub(super) fn rectangle(
    graph: &Graph<'_>,
    group: &[usize],
    stored: bool,
) -> Result<Plan, GeometryError> {
    rectangle_with_rotation(graph, group, stored, 0)
}
fn rectangle_with_rotation(
    graph: &Graph<'_>,
    group: &[usize],
    stored: bool,
    rotation: u8,
) -> Result<Plan, GeometryError> {
    let allowed = group
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let root = *group
        .iter()
        .min()
        .ok_or_else(|| failure("Empty SubD face pack"))?;
    let seed = Cell {
        face: root,
        rotation,
    };
    let mut seen = std::collections::HashSet::with_capacity(group.len());
    seen.insert(root);
    let mut row = VecDeque::from([seed]);
    for direction in [0, 2] {
        loop {
            let end = if direction == 0 {
                *row.back().unwrap()
            } else {
                *row.front().unwrap()
            };
            let Some(next) = graph.neighbor(end, direction, &allowed, stored) else {
                break;
            };
            if seen.contains(&next.face) {
                break;
            }
            seen.insert(next.face);
            if direction == 0 {
                row.push_back(next);
            } else {
                row.push_front(next);
            }
        }
    }
    let width = row.len();
    // A sheared torus can traverse every face in one coordinate direction.
    // Use the transverse direction to recover its two-dimensional grid.
    if width == group.len() && rotation == 0 && graph.neighbor(seed, 1, &allowed, stored).is_some()
    {
        return rectangle_with_rotation(graph, group, stored, 1);
    }
    let mut rows = VecDeque::from([row.into_iter().collect::<Vec<_>>()]);
    let mut skew = false;
    for direction in [1, 3] {
        loop {
            let edge = if direction == 1 {
                rows.back().unwrap()
            } else {
                rows.front().unwrap()
            };
            let candidates = edge
                .iter()
                .map(|&c| graph.neighbor(c, direction, &allowed, stored))
                .collect::<Vec<_>>();
            if candidates.iter().all(Option::is_none) {
                break;
            }
            if candidates.iter().any(Option::is_none) {
                return Err(failure("Nonrectangular SubD face pack"));
            }
            let candidates = candidates
                .into_iter()
                .map(Option::unwrap)
                .collect::<Vec<_>>();
            let count = candidates.iter().filter(|c| seen.contains(&c.face)).count();
            if count == width {
                let opposite = if direction == 1 {
                    rows.front().unwrap()
                } else {
                    rows.back().unwrap()
                };
                skew |= &candidates != opposite;
                break;
            }
            if count != 0 {
                return Err(failure("Overlapping SubD face-pack rows"));
            }
            for (i, c) in candidates.iter().enumerate() {
                if seen.contains(&c.face) {
                    return Err(failure("Repeated SubD face-pack cell"));
                }
                if i + 1 < width
                    && graph.neighbor(*c, 0, &allowed, stored) != Some(candidates[i + 1])
                {
                    return Err(failure("Inconsistent SubD face-pack charts"));
                }
                seen.insert(c.face);
            }
            if direction == 1 {
                rows.push_back(candidates);
            } else {
                rows.push_front(candidates);
            }
        }
    }
    if width * rows.len() != group.len() {
        return Err(failure("Incomplete rectangular SubD face pack"));
    }
    if !skew {
        let wrap_u = rows
            .iter()
            .all(|row| graph.physical(*row.last().unwrap(), 0, &allowed) == Some(row[0]));
        let wrap_v = (0..width).all(|x| {
            graph.physical(rows.back().unwrap()[x], 1, &allowed) == Some(rows.front().unwrap()[x])
        });
        // Native cuts use one cell along a wrapped V direction. A one-cell
        // strip retains the two-cell padding offset of its periodic net.
        if wrap_u && rows.len() == 1 {
            for row in &mut rows {
                row.rotate_right(2 % width);
            }
        }
        if wrap_v {
            let n = rows.len();
            rows.rotate_left((if width == 1 { 2 } else { 1 }) % n);
        }
    }
    let grid_size = [width, rows.len()];
    let tiles = rows
        .into_iter()
        .enumerate()
        .flat_map(|(y, row)| {
            row.into_iter()
                .enumerate()
                .map(move |(x, c)| SubDNurbsTile {
                    face: c.face,
                    corner: None,
                    cell: [x, y],
                    quarter_turns: c.rotation,
                })
        })
        .collect();
    Ok(Plan {
        grid_size,
        tiles,
        skew,
    })
}

pub(super) fn automatic(graph: &Graph<'_>, group: &[usize]) -> Result<Vec<Plan>, GeometryError> {
    let plan = rectangle(graph, group, false)?;
    if !plan.skew {
        return Ok(vec![plan]);
    }
    if plan.grid_size[1] < 2 {
        return Err(failure("Cannot unwrap sheared SubD face pack"));
    }
    let mut strip = Vec::new();
    let mut remaining = Vec::new();
    for tile in &plan.tiles {
        if tile.cell[1] == 1 {
            strip.push(tile.face);
        } else {
            remaining.push(tile.face);
        }
    }
    let first = rectangle(graph, &remaining, true)?;
    let second = rectangle(graph, &strip, true)?;
    if first.skew || second.skew {
        return Err(failure("Unresolved sheared SubD packing period"));
    }
    Ok(vec![first, second])
}

pub(super) fn stored(
    graph: &Graph<'_>,
    records: &[Option<viboceros_opennurbs::SubDFacePack>],
) -> Option<Vec<Plan>> {
    if records.len() != graph.source.faces().len() {
        return None;
    }
    let mut groups = std::collections::BTreeMap::<u32, Vec<usize>>::new();
    for (face, record) in records.iter().enumerate() {
        groups.entry(record.as_ref()?.id).or_default().push(face);
    }
    groups
        .into_values()
        .map(|faces| {
            if faces.len() == 1 && graph.source.faces()[faces[0]].len() != 4 {
                return Some(star(graph.source, faces[0]));
            }
            if faces.iter().any(|&f| graph.source.faces()[f].len() != 4) {
                return None;
            }
            let plan = rectangle(graph, &faces, true).ok()?;
            if plan.skew {
                return None;
            }
            Some(vec![plan])
        })
        .collect::<Option<Vec<_>>>()
        .map(|v| v.into_iter().flatten().collect())
}

pub(super) fn star(source: &SubD, face: usize) -> Vec<Plan> {
    (0..source.faces()[face].len())
        .map(|corner| Plan {
            grid_size: [1, 1],
            tiles: vec![SubDNurbsTile {
                face,
                corner: Some(corner),
                cell: [0, 0],
                quarter_turns: 0,
            }],
            skew: false,
        })
        .collect()
}
