//! Surface joining policy, separated from edge matching and document mutation.
use super::*;
use std::borrow::Cow;
use viboceros_geometry::join_breps;

pub(super) fn accepts(geometry: &Geometry) -> bool {
    matches!(geometry, Geometry::NurbsSurface(_) | Geometry::Brep(_))
}

fn brep(geometry: &Geometry, tolerance: Tolerance) -> Result<Cow<'_, Brep>, CommandError> {
    match geometry {
        Geometry::Brep(b) => Ok(Cow::Borrowed(b)),
        Geometry::NurbsSurface(s) => Ok(Cow::Owned(Brep::try_surface_face(s.clone(), tolerance)?)),
        _ => Err(CommandError::UnsupportedJoinGeometry),
    }
}

pub(super) fn closed(geometry: &Geometry, tolerance: Tolerance) -> Result<bool, CommandError> {
    Ok(brep(geometry, tolerance)?.is_closed())
}

pub(super) fn stage(
    sources: &[&viboceros_document::Object],
    tolerance: Tolerance,
    postselected: bool,
) -> Result<JoinPlan, CommandError> {
    let geometry = sources
        .iter()
        .map(|o| brep(o.geometry(), tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let mut release = Vec::new();
    let mut open = Vec::new();
    for (i, b) in geometry.iter().enumerate() {
        if b.is_closed() {
            release.push(sources[i].id());
        } else {
            open.push(i);
        }
    }
    if open.is_empty() {
        return Err(CommandError::NoOpenSurfacesToJoin);
    }
    if open.len() == 1 {
        return Err(CommandError::NothingJoined);
    }
    let distance = tolerance.absolute() * 2.;
    let (parts, consumed) = if postselected {
        let mut current = geometry[open[0]].as_ref().clone();
        let mut consumed = vec![sources[open[0]].id()];
        let mut final_parts = Vec::new();
        for &i in &open[1..] {
            let parts = join_breps(&[&current, geometry[i].as_ref()], distance, tolerance)?;
            if !parts
                .iter()
                .any(|p| p.source_indices.len() == 2 && p.joined_edge_count > 0)
            {
                continue;
            }
            current = Brep::try_combine(parts.iter().map(|p| p.brep.clone()).collect(), tolerance)?;
            consumed.push(sources[i].id());
            final_parts = parts
                .into_iter()
                .map(|p| (sources[open[0]].id(), Geometry::Brep(p.brep)))
                .collect();
        }
        if final_parts.is_empty() {
            return Err(CommandError::NothingJoined);
        }
        (final_parts, consumed)
    } else {
        let parts = join_breps(
            &open
                .iter()
                .map(|&i| geometry[i].as_ref())
                .collect::<Vec<_>>(),
            distance,
            tolerance,
        )?;
        let copies = parts
            .into_iter()
            .map(|p| {
                (
                    sources[open[p.source_indices[0]]].id(),
                    Geometry::Brep(p.brep),
                )
            })
            .collect();
        (copies, open.iter().map(|&i| sources[i].id()).collect())
    };
    Ok(JoinPlan {
        description: format!(
            "Joined {} surface object(s) into {} object(s)",
            consumed.len(),
            parts.len()
        ),
        copies: parts,
        consumed,
        closed_on_pick: false,
        release,
    })
}
