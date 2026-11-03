//! Native SubD contact policy feeds the shared atomic document edit.
use super::*;
pub(super) fn stage(
    sources: &[&viboceros_document::Object],
    tolerance: Tolerance,
    crease: bool,
    postselected: bool,
) -> Result<JoinPlan, CommandError> {
    let subds = sources
        .iter()
        .map(|o| {
            if let Geometry::SubD(s) = o.geometry() {
                s
            } else {
                unreachable!("preflighted SubD family")
            }
        })
        .collect::<Vec<_>>();
    let parts = if postselected {
        viboceros_geometry::join_subds_seeded(&subds, tolerance.absolute(), crease)?
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        viboceros_geometry::join_subds(&subds, tolerance.absolute(), crease)?
    };
    let unchanged = parts
        .iter()
        .filter(|p| p.source_indices.len() == 1)
        .map(|p| p.source_indices[0])
        .collect::<BTreeSet<_>>();
    let mut consumed = BTreeSet::new();
    let mut copies = Vec::new();
    for part in parts {
        if part.source_indices.len() < 2 {
            continue;
        }
        consumed.extend(
            part.source_indices
                .iter()
                .copied()
                .filter(|i| !unchanged.contains(i)),
        );
        copies.push((
            sources[part.source_indices[0]].id(),
            Geometry::SubD(part.subd),
        ));
    }
    let description = if copies.is_empty() {
        format!("{} SubD(s) unchanged", sources.len())
    } else {
        format!(
            "Joined {} SubD(s) into {} SubD(s)",
            consumed.len(),
            copies.len()
        )
    };
    Ok(JoinPlan {
        description,
        consumed: consumed.into_iter().map(|i| sources[i].id()).collect(),
        copies,
        release: vec![],
        closed_on_pick: false,
    })
}
