//! Contact-first setbacks on the picked side of two distinct open curves.
use super::*;
use crate::curve_pair_edit::{Plan, Target, closed, support};
use viboceros_document::ReplacementHistory;
use viboceros_geometry::{Curve3, LineSegment};
mod admission;
mod contacts;
mod setbacks;

pub(super) fn prepare(
    curves: [&Curve3; 2],
    picks: [Point3; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    let original_domains = curves.map(|c| c.as_ref().domain());
    let mut input = contacts::solve(curves, picks, options, tolerance)?;
    let result = setbacks::solve(&mut input, options, tolerance)?;
    let setbacks::Solution {
        mut contacts,
        consumed,
        invalid_join,
        reset_arc_domains: extra_reset,
    } = result;
    let curves = [&input.curves[0], &input.curves[1]];
    let seeds = input.seeds;
    let mut reset_arc_domains = input.reset_arc_domains;
    if options.join && options.trim && invalid_join {
        return Err(CommandError::Usage(
            "Chamfer setback would create an invalid joined curve",
        ));
    }
    for i in 0..2 {
        reset_arc_domains[i] = extra_reset[i]
            || (reset_arc_domains[i]
                && (options.distances[i] == 0.
                    || !original_domains[i].contains(&contacts.parameters[i])));
    }
    contacts.reset_arc_domains = reset_arc_domains;
    let mut plan = if options.trim && options.join && consumed.iter().any(|v| *v) {
        admission::consumed_join(curves, contacts, consumed, tolerance)?
    } else {
        closed::prepare(
            curves,
            seeds,
            [false, false],
            contacts,
            options.trim,
            options.join,
            tolerance,
        )?
    };
    if options.trim {
        for target in &mut plan.targets {
            if let Target::Source(i) = *target {
                *target = Target::SourceWithHistory(i, ReplacementHistory::EveryReplacement);
            }
        }
    }
    Ok(plan)
}
