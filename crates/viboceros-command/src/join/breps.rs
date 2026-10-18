//! Surface joining policy, separated from edge matching and document mutation.
use super::*;
use std::borrow::Cow;
#[cfg(not(feature = "native-smlib"))]
use viboceros_geometry::{BrepJoinComponent, join_breps, join_breps_with_report};
#[cfg(feature = "native-smlib")]
mod native;
#[cfg(feature = "native-smlib")]
pub(crate) use native::join_breps;
#[cfg(feature = "native-smlib")]
use native::join_breps_for_pick;

#[cfg(not(feature = "native-smlib"))]
fn join_breps_for_pick(
    sources: &[&Brep],
    distance: Real,
    tolerance: Tolerance,
) -> Result<(Vec<BrepJoinComponent>, bool), CommandError> {
    let report = join_breps_with_report(sources, distance, tolerance)?;
    let latest = sources.len().saturating_sub(1);
    let participates = report
        .candidate_source_pairs
        .iter()
        .any(|pair| pair.contains(&latest));
    Ok((report.components, participates))
}

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
    // Shared-input Rhino 8 command probes put the ordinary batch acceptance
    // radius below 1.8 model tolerances and command-first acceptance below 2.1.
    // Exact-cutoff endpoint behavior still differs; retain the kernel's
    // certified distance predicate rather than copying rounding artifacts.
    let radius = tolerance.absolute() * if postselected { 2.1 } else { 1.8 };
    let distance = if radius.is_finite() {
        radius.next_down()
    } else {
        // Preserve invalid-distance rejection rather than saturating overflow.
        radius
    };
    let (parts, consumed) = if postselected {
        let mut closed = false;
        let mut accepted = vec![geometry[open[0]].as_ref()];
        let mut consumed = vec![sources[open[0]].id()];
        let mut final_parts = Vec::new();
        for &i in &open[1..] {
            if closed {
                continue;
            }
            // Reconsider the original accepted boundaries, not an already
            // sewn temporary representation. A contacting pick can remain a
            // separate output after ambiguity resolution; unrelated picks
            // and picks that would remove every cross-source join are skipped.
            accepted.push(geometry[i].as_ref());
            let (parts, participates) = join_breps_for_pick(&accepted, distance, tolerance)?;
            if !participates || !parts.iter().any(|p| p.joined_edge_count > 0) {
                accepted.pop();
                continue;
            }
            closed = parts.iter().all(|part| part.brep.is_closed());
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
