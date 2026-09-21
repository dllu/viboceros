//! Surface joining policy, separated from edge matching and document mutation.
use super::*;
use std::borrow::Cow;
use viboceros_geometry::{join_breps, join_breps_with_report};

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
            let report = join_breps_with_report(&accepted, distance, tolerance)?;
            let latest = accepted.len() - 1;
            if !report
                .candidate_source_pairs
                .iter()
                .any(|p| p.contains(&latest))
                || !report.components.iter().any(|p| p.joined_edge_count > 0)
            {
                accepted.pop();
                continue;
            }
            let parts = report.components;
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
