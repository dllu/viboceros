//! Contact-first fillets: a closed parent is retained in full, never opened.
use super::{prepare::Plan, *};

pub(super) fn prepare(
    curves: [&Curve3; 2],
    picks: [Point3; 2],
    closed: [bool; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    let seed = [
        curves[0].as_ref().closest_parameter(picks[0], tolerance)?,
        curves[1].as_ref().closest_parameter(picks[1], tolerance)?,
    ];
    let plane = curves[usize::from(!closed[0])]
        .as_ref()
        .supporting_plane_frame(tolerance)?
        .ok_or(CommandError::Usage(
            "Closed-source fillet requires a planar contact",
        ))?;
    let solution = super::contacts::solve(curves, seed, options.radius, plane, tolerance)?;
    crate::curve_pair_edit::closed::prepare(
        curves,
        seed,
        closed,
        crate::curve_pair_edit::closed::Contacts {
            parameters: solution.parameters,
            points: solution.points,
            insertion: Some(Curve3::Arc(solution.arc)),
        },
        options.trim,
        options.join,
        tolerance,
    )
}
