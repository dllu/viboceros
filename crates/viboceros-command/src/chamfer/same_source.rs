//! Picked neighboring junctions retain the full parent intervals.
use super::*;
use crate::curve_pair_edit::{Plan, junction, leaf};
use viboceros_geometry::{Curve3, LineSegment};
mod components;

pub(super) fn prepare(
    source: &Curve3,
    picks: [Point3; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    let pieces = leaf::all(source, tolerance)?;
    let seeds = picks.map(|p| source.as_ref().closest_parameter(p, tolerance));
    let [a, b] = seeds;
    let seeds = [a?, b?];
    let index = |t| {
        pieces
            .iter()
            .position(|c| *c.as_ref().domain().end() > t)
            .unwrap_or(pieces.len() - 1)
    };
    let selected = seeds.map(index);
    let closed = source.as_ref().is_closed()?;
    let groups = components::groups(&pieces, closed, tolerance)?;
    let selected = selected.map(|i| groups[i]);
    if selected[0] == selected[1] {
        return Err(unavailable());
    }
    if closed && options.distances == [0., 0.] {
        return Err(unavailable());
    }
    let domain = source.as_ref().domain();
    let width = *domain.end() - *domain.start();
    let tie = 256.
        * Real::EPSILON
        * (*source.as_ref().domain().end() - *source.as_ref().domain().start());
    let mut best = None;
    for i in 0..pieces.len() - 1 + usize::from(closed) {
        let j = (i + 1) % pieces.len();
        if !((selected[0] == groups[i] && selected[1] == groups[j])
            || (selected[1] == groups[i] && selected[0] == groups[j]))
        {
            continue;
        }
        let (before, after) = (pieces[i].as_ref(), pieces[j].as_ref());
        let u = before
            .evaluate_with_tangent(*before.domain().end())?
            .tangent()
            .as_vector();
        let v = after
            .evaluate_with_tangent(*after.domain().start())?
            .tangent()
            .as_vector();
        if u.cross(v)?.length()? <= tolerance.angular().sin() {
            continue;
        }
        let forward = selected[0] == groups[i];
        let d = if forward {
            options.distances
        } else {
            [options.distances[1], options.distances[0]]
        };
        let point = |curve: viboceros_geometry::CurveRef<'_>,
                     distance,
                     from_end|
         -> Result<Option<(Real, Point3)>, CommandError> {
            if distance == 0. {
                let t = if from_end {
                    *curve.domain().end()
                } else {
                    *curve.domain().start()
                };
                return Ok(Some((t, curve.evaluate(t)?)));
            }
            Ok(curve
                .point_at_chord_distance(distance, from_end, tolerance)?
                .map(|s| (s.parameter, s.point)))
        };
        // Use the full prefix/suffix, so a setback can consume earlier spans.
        // Closed parents rotate to this junction and search both directions.
        let (before_search, after_search) = if closed {
            let seam = crate::curve_segments::seam(source, *before.domain().end())?;
            (seam.clone(), seam)
        } else {
            (
                crate::curve_segments::trimmed(source, *domain.start()..=*before.domain().end())?,
                crate::curve_segments::trimmed(source, *after.domain().start()..=*domain.end())?,
            )
        };
        let normalize = |(mut t, p): (Real, Point3)| {
            if closed {
                while t > *domain.end() {
                    t -= width;
                }
                while t < *domain.start() {
                    t += width;
                }
            }
            (t, p)
        };
        let a = if d[0] == 0. {
            point(before, 0., true)?
        } else {
            point(before_search.as_ref(), d[0], true)?.map(normalize)
        };
        let b = if d[1] == 0. {
            point(after, 0., false)?
        } else {
            point(after_search.as_ref(), d[1], false)?.map(normalize)
        };
        let (Some(a), Some(b)) = (a, b) else { continue };
        if !closed && (a.0 == *domain.start() || b.0 == *domain.end()) {
            continue;
        }
        let score = ((seeds[0] - a.0).abs() + (seeds[1] - b.0).abs())
            .min((seeds[0] - b.0).abs() + (seeds[1] - a.0).abs());
        if best.as_ref().is_none_or(|(old, _, _)| score < *old - tie) {
            best = Some((score, [a.0, b.0], [a.1, b.1]));
        }
    }
    let (_, parameters, points) = best.ok_or_else(unavailable)?;
    let insertion = if points[0].distance_to(points[1])? > tolerance.absolute() {
        Some(Curve3::Line(LineSegment::try_new(
            points[0],
            points[1],
            Tolerance::NUMERICAL_VALIDATION,
        )?))
    } else {
        None
    };
    junction::prepare(
        source,
        parameters,
        insertion,
        options.trim,
        options.join,
        tolerance,
        closed.then_some(parameters[0] < parameters[1]),
    )
}
fn unavailable() -> CommandError {
    CommandError::Usage("Unable to chamfer the picked curve pieces")
}
