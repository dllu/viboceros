//! Native bounded line supports, including style-dependent collinear contacts.
use super::{closed::Contacts, line_supports, support};
use crate::CommandError;
use viboceros_geometry::{
    Curve3, CurveCurveIntersectionEvent, CurveOtherExtensionStyle, Point3, Real, Tolerance,
};

pub(crate) struct Solution {
    pub contacts: Contacts,
    pub own_support_ends: bool,
}

pub(crate) fn solve(
    curves: [&Curve3; 2],
    seed: [Real; 2],
    style: CurveOtherExtensionStyle,
    join: bool,
    tolerance: Tolerance,
) -> Result<Option<Solution>, CommandError> {
    let Some(supports) = line_supports::curves(curves, style)? else {
        return Ok(None);
    };
    Ok(Some(from_supports(
        curves, seed, &supports, style, join, tolerance,
    )?))
}

pub(crate) fn from_supports(
    curves: [&Curve3; 2],
    seed: [Real; 2],
    supports: &[Curve3; 2],
    style: CurveOtherExtensionStyle,
    join: bool,
    tolerance: Tolerance,
) -> Result<Solution, CommandError> {
    let events = crate::curve_intersections::events(
        &supports[0].as_ref().to_nurbs()?,
        &supports[1].as_ref().to_nurbs()?,
        tolerance,
    )?;
    let mut best = None;
    for event in events {
        let hits = match event {
            CurveCurveIntersectionEvent::Point(hit) => [Some(hit), None],
            CurveCurveIntersectionEvent::Overlap(span) => {
                if style == CurveOtherExtensionStyle::Smooth {
                    // Smooth retains each source's own support endpoint. Rank
                    // the two aligned endpoint pairs using physical pick travel.
                    let same_direction =
                        span.start().second_parameter() < span.end().second_parameter();
                    let mut best = None;
                    for first_start in [true, false] {
                        let mut parameters = [0.; 2];
                        let mut points = [Point3::try_new(0., 0., 0.)?; 2];
                        let mut cost = 0.;
                        for i in 0..2 {
                            let c = supports[i].as_ref();
                            let d = c.domain();
                            let start = if i == 0 || same_direction {
                                first_start
                            } else {
                                !first_start
                            };
                            parameters[i] = if start { *d.start() } else { *d.end() };
                            points[i] = c.evaluate(parameters[i])?;
                            cost +=
                                support::distance(curves[i], seed[i], parameters[i], tolerance)?;
                        }
                        if best.as_ref().is_none_or(|(old, _, _)| cost < *old) {
                            best = Some((cost, parameters, points));
                        }
                    }
                    let (_, parameters, mut points) = best.unwrap();
                    if join {
                        points = [points[0].midpoint(points[1])?; 2];
                    }
                    return Ok(Solution {
                        own_support_ends: true,
                        contacts: Contacts {
                            parameters,
                            points,
                            insertion: None,
                            reset_arc_domains: [false; 2],
                            retain_end: None,
                        },
                    });
                }
                [Some(span.start()), Some(span.end())]
            }
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
    Ok(Solution {
        own_support_ends: false,
        contacts: Contacts {
            parameters,
            points: [point; 2],
            insertion: None,
            reset_arc_domains: [false; 2],
            retain_end: None,
        },
    })
}
