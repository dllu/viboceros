//! Retain closed parents in full and admit only their open-side contacts.
use super::*;
use viboceros_geometry::{LineSegment, PolyCurve3};
pub(crate) struct Contacts {
    pub parameters: [Real; 2],
    pub points: [Point3; 2],
    pub insertion: Option<Curve3>,
    pub reset_arc_domains: [bool; 2],
    /// Setback travel can pass the pick; keep the direction chosen at the corner.
    pub retain_end: Option<[bool; 2]>,
}
pub(crate) fn prepare(
    curves: [&Curve3; 2],
    seed: [Real; 2],
    closed: [bool; 2],
    contacts: Contacts,
    trim: bool,
    join: bool,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    let mut plan = Plan {
        outputs: Vec::new(),
        targets: Vec::new(),
        deleted: Vec::new(),
    };
    if contacts.insertion.is_none() && !trim {
        return Ok(plan);
    }
    if contacts.insertion.is_none() && closed == [true, true] {
        return Err(CommandError::Usage(
            "Cannot construct a zero-length chamfer",
        ));
    }
    let mut joined = Vec::new();
    if trim {
        for i in 0..2 {
            if closed[i] {
                continue;
            }
            let contact = contacts.parameters[i];
            let domain = curves[i].as_ref().domain();
            let keep_end = contacts
                .retain_end
                .map_or(seed[i] > contact, |ends| ends[i]);
            let interval = if keep_end {
                contact..=*domain.end()
            } else {
                *domain.start()..=contact
            };
            let mut retained = if let Curve3::Line(line) = curves[i] {
                // A line support can reach the contact outside its old domain;
                // retain the far endpoint and the original parameter chart.
                let (a, b) = if keep_end {
                    (contacts.points[i], line.end())
                } else {
                    (line.start(), contacts.points[i])
                };
                Curve3::Line(
                    LineSegment::try_new(a, b, Tolerance::NUMERICAL_VALIDATION)?
                        .try_reparameterized(interval)?,
                )
            } else {
                crate::curve_segments::trimmed(curves[i], interval)?
            };
            if contacts.reset_arc_domains[i]
                && let Curve3::Arc(arc) = &mut retained
            {
                *arc = arc.try_reparameterized(0. ..=arc.length()?)?;
            }
            if join {
                let retained = if keep_end == (i == 0) {
                    retained.reversed(tolerance)?
                } else {
                    retained
                };
                if i == 1
                    && let Some(c) = &contacts.insertion
                {
                    joined.extend(c.to_polycurve()?.segments().iter().cloned());
                }
                joined.extend(retained.to_polycurve()?.segments().iter().cloned());
            } else {
                plan.outputs.push(retained.into());
                plan.targets.push(Target::Source(i));
            }
        }
    }
    if trim && join {
        if closed[1]
            && let Some(c) = &contacts.insertion
        {
            joined.extend(c.to_polycurve()?.segments().iter().cloned());
        }
        plan.outputs
            .push(Geometry::PolyCurve(PolyCurve3::try_new(joined)?));
        plan.targets.push(if closed[0] {
            Target::FirstAttributes
        } else {
            Target::Source(0)
        });
        plan.deleted = (1..2).filter(|&i| !closed[i]).collect();
    } else {
        if let Some(c) = contacts.insertion {
            plan.outputs.push(c.into());
            plan.targets.push(Target::New);
        }
    }
    Ok(plan)
}
