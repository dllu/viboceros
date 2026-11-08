//! Contact-first fillets: a closed parent is retained in full, never opened.
use super::{prepare::Plan, *};
mod continuation;
mod zero;

pub(super) fn prepare(
    curves: [&Curve3; 2],
    picks: [Point3; 2],
    closed: [bool; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    if options.radius == 0. {
        return zero::prepare(curves, picks, closed, options, tolerance);
    }
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
    let mut candidate = super::contacts::solve(curves, seed, options.radius, plane, tolerance)?
        .map(|contacts| continuation::Candidate {
            curves: curves.map(Clone::clone),
            contacts,
            reset_arc_domains: [false; 2],
        });
    if let Some(other) = continuation::solve(curves, seed, closed, options, plane, tolerance)? {
        let replace = match &candidate {
            Some(old) => {
                continuation::cost(&other, seed, tolerance)?
                    < continuation::cost(old, seed, tolerance)?
            }
            None => true,
        };
        if replace {
            candidate = Some(other);
        }
    }
    let Some(candidate) = candidate else {
        return Ok(Plan {
            outputs: Vec::new(),
            targets: Vec::new(),
            deleted: Vec::new(),
        });
    };
    admit(candidate, seed, closed, options, tolerance)
}

fn admit(
    candidate: continuation::Candidate,
    seed: [Real; 2],
    closed: [bool; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    if options.trim {
        for i in 0..2 {
            if closed[i] {
                continue;
            }
            let curve = &candidate.curves[i];
            let t = candidate.contacts.parameters[i];
            let tangent = if let Curve3::Line(line) = curve {
                line.direction(tolerance)?
            } else {
                curve.as_ref().evaluate_with_tangent(t)?.tangent()
            };
            let arc = &candidate.contacts.arc;
            let domain = arc.domain();
            let arc_tangent = viboceros_geometry::CurveRef::Arc(arc)
                .evaluate_with_tangent(if i == 0 {
                    *domain.start()
                } else {
                    *domain.end()
                })?
                .tangent();
            let keep_end = seed[i] > t;
            let sign = if keep_end == (i == 0) { -1. } else { 1. };
            if sign * tangent.as_vector().dot(arc_tangent.as_vector())? < 0. {
                return Err(CommandError::Usage(
                    "Fillet tangent opposes the retained curve",
                ));
            }
        }
    }
    crate::curve_pair_edit::closed::prepare(
        [&candidate.curves[0], &candidate.curves[1]],
        seed,
        closed,
        crate::curve_pair_edit::closed::Contacts {
            parameters: candidate.contacts.parameters,
            points: candidate.contacts.points,
            insertion: Some(Curve3::Arc(candidate.contacts.arc)),
            reset_arc_domains: candidate.reset_arc_domains,
            retain_end: None,
        },
        options.trim,
        options.join,
        tolerance,
    )
}
