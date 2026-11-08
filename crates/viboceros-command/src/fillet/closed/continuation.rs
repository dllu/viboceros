//! Solve contacts on source continuations without admitting unused extensions.
use super::*;
use crate::fillet::contacts;
use viboceros_geometry::Frame3;
mod circular;
mod linear;
mod search;
mod smooth;
mod tail;

pub(super) fn cost(
    candidate: &Candidate,
    seed: [Real; 2],
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    search::score(
        [&candidate.curves[0], &candidate.curves[1]],
        seed,
        &candidate.contacts,
        tolerance,
    )
}

pub(super) struct Candidate {
    pub curves: [Curve3; 2],
    pub contacts: contacts::Contacts,
    pub reset_arc_domains: [bool; 2],
}

pub(super) fn solve(
    original: [&Curve3; 2],
    seed: [Real; 2],
    closed: [bool; 2],
    options: Preferences,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Option<Candidate>, CommandError> {
    let Some(i) = closed.iter().position(|closed| !closed) else {
        return Ok(None);
    };
    if matches!(original[i], Curve3::Line(_)) {
        return Ok(None);
    }
    if matches!(original[i], Curve3::Arc(_)) && options.styles.arc == CurveArcExtensionStyle::Arc {
        circular::solve(original, seed, i, options.radius, plane, tolerance)
    } else if matches!(original[i], Curve3::Arc(_))
        || options.styles.other == CurveOtherExtensionStyle::Line
    {
        linear::solve(original, seed, i, options.radius, plane, tolerance)
    } else {
        smooth::solve(original, seed, i, options.radius, plane, tolerance)
    }
}
