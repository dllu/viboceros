//! Contact-first fillets: a closed parent is retained in full, never opened.
use super::{
    prepare::{Plan, Target},
    *,
};
use viboceros_geometry::{CurveSegment3, LineSegment, PolyCurve3};

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
    let arc = solution.arc;
    let mut plan = Plan {
        outputs: Vec::new(),
        targets: Vec::new(),
        deleted: Vec::new(),
    };
    let mut joined = Vec::new();
    if options.trim {
        for i in 0..2 {
            if closed[i] {
                continue;
            }
            let contact = solution.parameters[i];
            let domain = curves[i].as_ref().domain();
            let keep_end = seed[i] > contact;
            let interval = if keep_end {
                contact..=*domain.end()
            } else {
                *domain.start()..=contact
            };
            let retained = if let Curve3::Line(line) = curves[i] {
                // A line support can reach the contact outside its old domain;
                // retain the far endpoint and the original parameter chart.
                let (a, b) = if keep_end {
                    (solution.points[i], line.end())
                } else {
                    (line.start(), solution.points[i])
                };
                Curve3::Line(
                    LineSegment::try_new(a, b, Tolerance::NUMERICAL_VALIDATION)?
                        .try_reparameterized(interval)?,
                )
            } else {
                super::super::curve_segments::trimmed(curves[i], interval)?
            };
            if options.join {
                let retained = if keep_end == (i == 0) {
                    retained.reversed(tolerance)?
                } else {
                    retained
                };
                if i == 1 {
                    joined.push(CurveSegment3::Arc(arc));
                }
                joined.extend(retained.to_polycurve()?.segments().iter().cloned());
            } else {
                plan.outputs.push(retained.into());
                plan.targets.push(Target::Source(i));
            }
        }
    }
    if options.trim && options.join {
        if closed[1] {
            joined.push(CurveSegment3::Arc(arc));
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
        plan.outputs.push(Geometry::Arc(arc));
        plan.targets.push(Target::New);
    }
    Ok(plan)
}
