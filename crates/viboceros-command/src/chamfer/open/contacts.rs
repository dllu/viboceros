use super::*;
use crate::curve_pair_edit::{boundary_continuation, line_contacts};
use viboceros_geometry::CurveCurveIntersectionEvent;

pub(super) struct Solution {
    pub curves: [Curve3; 2],
    pub seeds: [Real; 2],
    pub corners: [Real; 2],
    pub points: [Point3; 2],
    pub reset_arc_domains: [bool; 2],
    pub smooth_overlap: bool,
}
pub(super) fn solve(
    curves: [&Curve3; 2],
    picks: [Point3; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Solution, CommandError> {
    let mut working = curves.map(Clone::clone);
    let mut reset = [false; 2];
    let seeds = [
        curves[0].as_ref().closest_parameter(picks[0], tolerance)?,
        curves[1].as_ref().closest_parameter(picks[1], tolerance)?,
    ];
    if let Some(solved) = line_contacts::solve(
        curves,
        seeds,
        options.styles.other,
        options.join && options.distances == [0., 0.],
        tolerance,
    )? {
        let mut contacts = solved.contacts;
        for (i, &curve) in curves.iter().enumerate() {
            for t in [
                *curve.as_ref().domain().start(),
                *curve.as_ref().domain().end(),
            ] {
                let endpoint = curve.as_ref().evaluate(t)?;
                if endpoint.distance_to(contacts.points[i])? <= tolerance.absolute() {
                    contacts.parameters[i] = t;
                    contacts.points[i] = endpoint;
                    break;
                }
            }
        }
        return Ok(Solution {
            curves: working,
            seeds,
            corners: contacts.parameters,
            points: contacts.points,
            reset_arc_domains: reset,
            smooth_overlap: solved.own_support_ends,
        });
    }
    let mut events = support::intersections(curves, tolerance)?;
    if events.is_empty() {
        for i in 0..2 {
            if !matches!(curves[i], Curve3::Line(_)) {
                let boundary = support::curve(curves[1 - i], curves[i], tolerance)?;
                working[i] = boundary_continuation::to_boundary(
                    curves[i],
                    &boundary,
                    picks[i],
                    boundary_continuation::Styles {
                        arc: options.styles.arc,
                        other: options.styles.other,
                    },
                    tolerance,
                )?;
                reset[i] = matches!(&working[i], Curve3::Arc(_));
            }
        }
        events = support::intersections([&working[0], &working[1]], tolerance)?;
    }
    let curves = [&working[0], &working[1]];
    let seeds = [
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
            let mut parameters = [0.; 2];
            let mut cost = 0.;
            for i in 0..2 {
                parameters[i] = support::parameter(curves[i], hit.point(), tolerance)?;
                for t in [
                    *curves[i].as_ref().domain().start(),
                    *curves[i].as_ref().domain().end(),
                ] {
                    if curves[i].as_ref().evaluate(t)?.distance_to(hit.point())?
                        <= tolerance.absolute()
                    {
                        parameters[i] = t;
                        break;
                    }
                }
                cost += support::distance(curves[i], seeds[i], parameters[i], tolerance)?;
            }
            if best.as_ref().is_none_or(|(old, _, _)| cost < *old) {
                best = Some((cost, parameters, hit.point()));
            }
        }
    }
    let (_, corners, point) = best.ok_or(CommandError::Usage(
        "No curve intersection satisfies these picks",
    ))?;
    Ok(Solution {
        curves: working,
        seeds,
        corners,
        points: [point; 2],
        reset_arc_domains: reset,
        smooth_overlap: false,
    })
}
