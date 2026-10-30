//! Legalize a constrained planar triangulation using the original world metric.
use super::regions::charge;
use crate::{GeometryError, Point3};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, VecDeque},
};
mod predicates;
#[cfg(test)]
mod tests;

type Edge = [u32; 2];
fn edge(a: u32, b: u32) -> Edge {
    [a.min(b), a.max(b)]
}
fn edges([a, b, c]: [u32; 3]) -> [Edge; 3] {
    [edge(a, b), edge(b, c), edge(c, a)]
}
fn invalid() -> GeometryError {
    GeometryError::Degenerate {
        context: "SubD cap triangulation incidence",
    }
}

fn attach(
    map: &mut BTreeMap<Edge, Vec<usize>>,
    face: usize,
    triangle: [u32; 3],
) -> Result<(), GeometryError> {
    for key in edges(triangle) {
        let uses = map.entry(key).or_default();
        if uses.len() >= 2 || uses.contains(&face) {
            return Err(invalid());
        }
        uses.push(face);
    }
    Ok(())
}
fn detach(
    map: &mut BTreeMap<Edge, Vec<usize>>,
    face: usize,
    triangle: [u32; 3],
) -> Result<(), GeometryError> {
    for key in edges(triangle) {
        let uses = map.get_mut(&key).ok_or_else(invalid)?;
        let i = uses.iter().position(|&f| f == face).ok_or_else(invalid)?;
        uses.swap_remove(i);
        if uses.is_empty() {
            map.remove(&key);
        }
    }
    Ok(())
}

pub(super) fn legalize(
    points: &[Point3],
    axes: [usize; 2],
    faces: &mut [[u32; 3]],
    work: &mut usize,
) -> Result<(), GeometryError> {
    if faces.iter().flatten().any(|&v| v as usize >= points.len()) {
        return Err(invalid());
    }
    let mut map = BTreeMap::new();
    for (i, &t) in faces.iter().enumerate() {
        attach(&mut map, i, t)?;
    }
    // Constraint boundaries have one incident cap triangle and are never
    // flipped. Only strictly illegal diagonals change; exact circle ties keep
    // the existing layout and its separately qualified behavior.
    let mut queue = map
        .iter()
        .filter_map(|(&key, uses)| (uses.len() == 2).then_some(key))
        .collect::<VecDeque<_>>();
    let mut pending = queue.iter().copied().collect::<BTreeSet<_>>();
    while let Some(key) = queue.pop_front() {
        pending.remove(&key);
        charge(work, 1)?;
        let Some(uses) = map.get(&key).filter(|uses| uses.len() == 2) else {
            continue;
        };
        let (f, g) = (uses[0], uses[1]);
        let Some(&c) = faces[f].iter().find(|v| !key.contains(v)) else {
            return Err(invalid());
        };
        let Some(&d) = faces[g].iter().find(|v| !key.contains(v)) else {
            return Err(invalid());
        };
        let [mut a, mut b] = key;
        let p = |v: u32| points[v as usize];
        match predicates::orient([p(a), p(b), p(c)], axes) {
            Ordering::Less => std::mem::swap(&mut a, &mut b),
            Ordering::Equal => return Err(invalid()),
            Ordering::Greater => {}
        }
        if c == d
            || predicates::orient([p(c), p(d), p(b)], axes) != Ordering::Greater
            || predicates::orient([p(d), p(c), p(a)], axes) != Ordering::Greater
        {
            continue;
        }
        if predicates::circle([p(a), p(b), p(c), p(d)], axes) != Ordering::Greater {
            continue;
        }
        detach(&mut map, f, faces[f])?;
        detach(&mut map, g, faces[g])?;
        faces[f] = [c, d, b];
        faces[g] = [d, c, a];
        attach(&mut map, f, faces[f])?;
        attach(&mut map, g, faces[g])?;
        for key in edges(faces[f]).into_iter().chain(edges(faces[g])) {
            if pending.insert(key) {
                queue.push_back(key);
            }
        }
    }
    Ok(())
}
