//! A same-parent fillet edits one source, preserving the unconsumed intervals.
use super::{prepare::Plan, *};
mod select;

pub(super) fn prepare(
    source: &Curve3,
    picks: [Point3; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    if options.radius == 0. {
        return Err(CommandError::Usage(
            "Unable to intersect curves or curve extensions",
        ));
    }
    let Some((parameters, arc)) = select::candidate(source, picks, options, tolerance)? else {
        return Ok(Plan {
            outputs: Vec::new(),
            targets: Vec::new(),
            deleted: Vec::new(),
        });
    };
    crate::curve_pair_edit::junction::prepare(
        source,
        parameters,
        Some(Curve3::Arc(arc)),
        options.trim,
        options.join,
        tolerance,
        None,
    )
}
