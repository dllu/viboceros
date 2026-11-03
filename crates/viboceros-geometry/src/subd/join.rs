//! Contact discovery is separate from ordered admission and native assembly.
use super::*;
use std::collections::{BTreeSet, HashMap, HashSet};
mod contacts;
mod state;
#[cfg(test)]
mod tests;
use contacts::Contacts;
use state::Assembly;

#[derive(Clone, Debug)]
pub struct SubDJoinResult {
    pub subd: SubD,
    /// Contributing inputs; recorded nonmanifold competition can also return
    /// an unchanged copy of an input listed in another result.
    pub source_indices: Vec<usize>,
}

fn error(message: &str) -> GeometryError {
    viboceros_opennurbs::SubDError(message.into()).into()
}

/// Batch joining reconsidering connected inputs, with unchanged copies returned
/// separately. The sources are immutable; tolerance measures control positions.
pub fn join_subds(
    sources: &[&SubD],
    tolerance: f64,
    crease: bool,
) -> Result<Vec<SubDJoinResult>, GeometryError> {
    let contacts = Contacts::new(sources, tolerance)?;
    if sources.len() < 2 {
        return Ok(vec![]);
    }
    let mut consumed = vec![false; sources.len()];
    let mut unchanged = BTreeSet::new();
    let mut joined = Vec::new();
    for seed in 0..sources.len() {
        if consumed[seed] {
            continue;
        }
        consumed[seed] = true;
        let mut assembly = Assembly::new(sources, seed)?;
        let mut pending = contacts.neighbors[seed].clone();
        while let Some(next) = pending.pop_first() {
            if consumed[next] {
                continue;
            }
            if let Some(residual) = assembly.append(next, &contacts, tolerance, true)? {
                consumed[next] = true;
                if residual {
                    unchanged.insert(next);
                }
                pending.extend(contacts.neighbors[next].iter().copied());
            }
        }
        if assembly.order.len() == 1 {
            unchanged.insert(seed);
        } else {
            joined.push(assembly.finish(crease)?);
        }
    }
    let mut results = unchanged
        .into_iter()
        .map(|i| SubDJoinResult {
            subd: sources[i].clone(),
            source_indices: vec![i],
        })
        .collect::<Vec<_>>();
    results.extend(joined);
    Ok(results)
}

/// Command-first picks extend only the seed, once per pick. Interior edge
/// competition is skipped; an unrelated earlier pick is never reconsidered.
pub fn join_subds_seeded(
    sources: &[&SubD],
    tolerance: f64,
    crease: bool,
) -> Result<Option<SubDJoinResult>, GeometryError> {
    let contacts = Contacts::new(sources, tolerance)?;
    if sources.len() < 2 {
        return Ok(None);
    }
    let mut assembly = Assembly::new(sources, 0)?;
    for next in 1..sources.len() {
        assembly.append(next, &contacts, tolerance, false)?;
    }
    if assembly.order.len() == 1 {
        Ok(None)
    } else {
        assembly.finish(crease).map(Some)
    }
}
