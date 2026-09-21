//! Mesh joining stages raw-index-preserving kernel results.
use super::*;

pub(super) fn stage(
    sources: &[&viboceros_document::Object],
    tolerance: Tolerance,
    disjoint: bool,
) -> Result<JoinPlan, CommandError> {
    if sources.len() == 1 {
        return Ok(JoinPlan {
            copies: vec![],
            consumed: vec![],
            description: "One mesh unchanged".into(),
            closed_on_pick: false,
        });
    }
    let meshes = sources
        .iter()
        .map(|o| match o.geometry() {
            Geometry::Mesh(m) => m,
            _ => unreachable!("preflighted mesh family"),
        })
        .collect::<Vec<_>>();
    let components = viboceros_geometry::join_meshes(
        &meshes,
        viboceros_geometry::MeshJoinOptions {
            join_disjoint: disjoint,
            alignment_tolerance: tolerance.absolute() * 1e-4,
            single_precision_matching: true,
        },
    )?;
    let count = components.len();
    Ok(JoinPlan {
        copies: components
            .into_iter()
            .map(|part| {
                (
                    sources[part.source_indices[0]].id(),
                    Geometry::Mesh(part.mesh),
                )
            })
            .collect(),
        consumed: sources.iter().map(|o| o.id()).collect(),
        description: format!("Joined {} mesh(es) into {count} mesh(es)", sources.len()),
        closed_on_pick: false,
    })
}
