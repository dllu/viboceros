//! Native bounded line supports, including style-dependent collinear contacts.
use super::*;
use viboceros_geometry::{BoundingBox3, LineSegment};

pub(super) fn solve(
    curves: [&Curve3; 2],
    seed: [Real; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Option<Contacts>, CommandError> {
    let [Curve3::Line(first), Curve3::Line(second)] = curves else {
        return Ok(None);
    };
    let bounds =
        BoundingBox3::from_points([first.start(), first.end(), second.start(), second.end()])?;
    let padding = 100. * bounds.min().distance_to(bounds.max())?;
    let mut supports = Vec::with_capacity(2);
    for line in [first, second] {
        // Native Line uses a pair-bounds distance; Smooth uses source spans.
        let fraction = if options.styles.other == CurveOtherExtensionStyle::Smooth {
            50.
        } else {
            padding / line.length()?
        };
        let domain = line.domain();
        let width = *domain.end() - *domain.start();
        supports.push(Curve3::Line(
            LineSegment::try_new(
                line.point_at(-fraction)?,
                line.point_at(1. + fraction)?,
                Tolerance::NUMERICAL_VALIDATION,
            )?
            .try_reparameterized(
                *domain.start() - fraction * width..=*domain.end() + fraction * width,
            )?,
        ));
    }
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
                if options.styles.other == CurveOtherExtensionStyle::Smooth {
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
                    if options.join {
                        points = [points[0].midpoint(points[1])?; 2];
                    }
                    return Ok(Some(Contacts {
                        parameters,
                        points,
                        insertion: None,
                        reset_arc_domains: [false; 2],
                        retain_end: None,
                    }));
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
    Ok(Some(Contacts {
        parameters,
        points: [point; 2],
        insertion: None,
        reset_arc_domains: [false; 2],
        retain_end: None,
    }))
}
