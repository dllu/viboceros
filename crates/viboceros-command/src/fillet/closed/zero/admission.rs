//! Normalize endpoint contacts and retain explicit zero-radius replacements in history.
use super::*;
use crate::curve_pair_edit::Target;
use viboceros_document::ReplacementHistory;

pub(super) fn prepare(
    curves: [&Curve3; 2],
    seed: [Real; 2],
    closed: [bool; 2],
    mut contacts: Contacts,
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    if closed == [true, true] {
        if options.join {
            // No open piece and no connector can form a valid joined curve.
            return Err(CommandError::Usage(
                "Zero-radius joining requires an open curve",
            ));
        }
        return Ok(Plan {
            outputs: Vec::new(),
            targets: Vec::new(),
            deleted: Vec::new(),
        });
    }
    for i in 0..2 {
        if closed[i] {
            continue;
        }
        let source = curves[i].as_ref();
        let domain = source.domain();
        for t in [*domain.start(), *domain.end()] {
            let endpoint = source.evaluate(t)?;
            if endpoint.distance_to(contacts.points[i])? <= tolerance.absolute() {
                contacts.parameters[i] = t;
                contacts.points[i] = endpoint;
                break;
            }
        }
    }
    let mut plan = crate::curve_pair_edit::closed::prepare(
        curves,
        seed,
        closed,
        contacts,
        options.trim,
        options.join,
        tolerance,
    )?;
    for target in &mut plan.targets {
        if let Target::Source(i) = *target {
            *target = Target::SourceWithHistory(i, ReplacementHistory::EveryReplacement);
        }
    }
    Ok(plan)
}
