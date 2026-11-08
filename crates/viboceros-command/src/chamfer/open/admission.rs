//! Joining omits a completely consumed piece but preserves its source identity.
use super::*;
use crate::curve_pair_edit::closed::Contacts;
use viboceros_geometry::PolyCurve3;

pub(super) fn consumed_join(
    curves: [&Curve3; 2],
    contacts: Contacts,
    consumed: [bool; 2],
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    let retain_end = contacts.retain_end.expect("open setback directions");
    let mut segments = Vec::new();
    for i in 0..2 {
        if i == 1
            && let Some(bevel) = &contacts.insertion
        {
            segments.extend(bevel.to_polycurve()?.segments().iter().cloned());
        }
        if consumed[i] {
            continue;
        }
        let retained = support::retained(
            curves[i],
            contacts.parameters[i],
            contacts.points[i],
            retain_end[i],
        )?;
        let retained = if retain_end[i] == (i == 0) {
            retained.reversed(tolerance)?
        } else {
            retained
        };
        segments.extend(retained.to_polycurve()?.segments().iter().cloned());
    }
    Ok(Plan {
        outputs: vec![Geometry::PolyCurve(PolyCurve3::try_new(segments)?)],
        targets: vec![Target::Source(0)],
        deleted: vec![1],
    })
}
