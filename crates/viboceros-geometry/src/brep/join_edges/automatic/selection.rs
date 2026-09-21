//! Mutual unique matches first, then unambiguous closure within components.
use super::*;

pub(super) fn pairs(
    brep: &Brep,
    candidates: &[(Real, usize, usize, bool)],
    split: &[bool],
) -> Vec<(usize, usize, bool)> {
    let mut faces = vec![None; brep.edges.len()];
    let mut components = (0..brep.faces.len()).collect::<Vec<_>>();
    for usage in brep.trim_uses() {
        if let Some(edge) = usage.trim.edge {
            if let Some(first) = faces[edge] {
                unite(&mut components, first, usage.face);
            } else {
                faces[edge] = Some(usage.face);
            }
        }
    }
    let mut used = vec![false; brep.edges.len()];
    let mut chosen = Vec::new();
    for closure in [false, true] {
        let mut best = vec![None::<(usize, bool)>; brep.edges.len()];
        for (i, &(_, a, b, _)) in candidates.iter().enumerate() {
            if used[a]
                || used[b]
                || (closure
                    && root(&mut components, faces[a].unwrap())
                        != root(&mut components, faces[b].unwrap()))
            {
                continue;
            }
            for edge in [a, b] {
                match &mut best[edge] {
                    Some((_, unique)) => *unique = false,
                    choice => *choice = Some((i, true)),
                }
            }
        }
        for (i, &(_, a, b, reversed)) in candidates.iter().enumerate() {
            if [a, b]
                .into_iter()
                .all(|e| best[e].is_some_and(|(index, unique)| index == i && unique))
            {
                used[a] = true;
                used[b] = true;
                unite(&mut components, faces[a].unwrap(), faces[b].unwrap());
                // A complete boundary owns its representation ahead of a
                // newly subdivided one. If both were cut, the later source's
                // piece is retained. Deferred component closure also keeps
                // the later boundary. The explicit assembly API stays neutral.
                chosen.push(if split[a] || closure {
                    (b, a, reversed)
                } else {
                    (a, b, reversed)
                });
            }
        }
    }
    chosen
}

fn root(roots: &mut [usize], mut i: usize) -> usize {
    while roots[i] != i {
        roots[i] = roots[roots[i]];
        i = roots[i];
    }
    i
}
fn unite(roots: &mut [usize], a: usize, b: usize) {
    let a = root(roots, a);
    let b = root(roots, b);
    roots[a.max(b)] = a.min(b);
}
