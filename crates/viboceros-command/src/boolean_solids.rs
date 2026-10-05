//! Shared ID-set parsing and original-face coplanar merging.
use super::*;

pub(super) fn parse_ids(value: &str, usage: &'static str) -> Result<Vec<ObjectId>, CommandError> {
    let ids = value
        .split(',')
        .map(str::parse)
        .collect::<Result<Vec<ObjectId>, _>>()
        .map_err(|_| CommandError::Usage(usage))?;
    if ids.is_empty() || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(CommandError::Usage(usage));
    }
    Ok(ids)
}
pub(super) fn merged(
    brep: Brep,
    sources: &[[usize; 2]],
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let mut labels = BTreeMap::new();
    let groups = sources
        .iter()
        .map(|source| {
            let next = labels.len();
            *labels.entry(*source).or_insert(next)
        })
        .collect::<Vec<_>>();
    brep.try_merge_coplanar_polygon_faces_in_groups(&groups, tolerance)?
        .unwrap_or(brep)
        .try_merge_all_edges(0., tolerance)
}
