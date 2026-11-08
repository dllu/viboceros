//! Zero radius intersects supports and retains only the picked open portion.
use super::{prepare::Plan, *};
use crate::curve_pair_edit::{closed::Contacts, support};
use viboceros_geometry::CurveCurveIntersectionEvent;
mod admission;
mod lines;

pub(super) fn prepare(
    curves: [&Curve3; 2],
    picks: [Point3; 2],
    closed: [bool; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    if !options.trim {
        return Ok(Plan {
            outputs: Vec::new(),
            targets: Vec::new(),
            deleted: Vec::new(),
        });
    }
    let seed = [
        curves[0].as_ref().closest_parameter(picks[0], tolerance)?,
        curves[1].as_ref().closest_parameter(picks[1], tolerance)?,
    ];
    if closed == [false, false]
        && let Some(contacts) = lines::solve(curves, seed, options, tolerance)?
    {
        return admission::prepare(curves, seed, closed, contacts, options, tolerance);
    }
    let mut working = curves.map(Clone::clone);
    let mut events = support::intersections([&working[0], &working[1]], tolerance)?;
    let mut reset_arc_domains = [false; 2];
    if events.is_empty() {
        for i in 0..2 {
            if !closed[i] && !matches!(curves[i], Curve3::Line(_)) {
                use crate::curve_pair_edit::boundary_continuation::{self, Styles};
                let boundary = support::curve(curves[1 - i], curves[i], tolerance)?;
                working[i] = boundary_continuation::to_boundary(
                    curves[i],
                    &boundary,
                    picks[i],
                    Styles {
                        arc: options.styles.arc,
                        other: options.styles.other,
                    },
                    tolerance,
                )?;
                reset_arc_domains[i] = matches!(&working[i], Curve3::Arc(_));
            }
        }
        events = support::intersections([&working[0], &working[1]], tolerance)?;
    }
    let curves = [&working[0], &working[1]];
    let seed = [
        curves[0].as_ref().closest_parameter(picks[0], tolerance)?,
        curves[1].as_ref().closest_parameter(picks[1], tolerance)?,
    ];
    let mut best = None;
    for event in events {
        let hits = match event {
            CurveCurveIntersectionEvent::Point(hit) => [Some(hit), None],
            CurveCurveIntersectionEvent::Overlap(span) => [Some(span.start()), Some(span.end())],
        };
        for hit in hits.into_iter().flatten() {
            let parameters = [
                support::parameter(curves[0], hit.point(), tolerance)?,
                support::parameter(curves[1], hit.point(), tolerance)?,
            ];
            let cost = support::distance(curves[0], seed[0], parameters[0], tolerance)?
                + support::distance(curves[1], seed[1], parameters[1], tolerance)?;
            if best.as_ref().is_none_or(|(old, _, _)| cost < *old) {
                best = Some((cost, parameters, hit.point()));
            }
        }
    }
    let (_, parameters, point) = best.ok_or(CommandError::Usage(
        "Unable to intersect curves or curve extensions",
    ))?;
    admission::prepare(
        curves,
        seed,
        closed,
        Contacts {
            parameters,
            points: [point; 2],
            insertion: None,
            reset_arc_domains,
            retain_end: None,
        },
        options,
        tolerance,
    )
}
