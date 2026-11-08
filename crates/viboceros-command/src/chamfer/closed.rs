//! Intersection-first setbacks keep every closed source in full.
use super::*;
use crate::curve_pair_edit::{
    Plan,
    closed::{self, Contacts},
};
use viboceros_geometry::{Curve3, CurveCurveIntersectionEvent, LineSegment};
mod support;

pub(super) fn prepare(
    curves: [&Curve3; 2],
    picks: [Point3; 2],
    closed: [bool; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    let a = curves[0].as_ref().closest_parameter(picks[0], tolerance)?;
    let b = curves[1].as_ref().closest_parameter(picks[1], tolerance)?;
    let seeds = [a, b];
    let supports = [
        support::curve(curves[0], curves[1], tolerance)?,
        support::curve(curves[1], curves[0], tolerance)?,
    ];
    let events = crate::curve_intersections::events(
        &supports[0].as_ref().to_nurbs()?,
        &supports[1].as_ref().to_nurbs()?,
        tolerance,
    )?;
    let mut best = None;
    for event in events {
        let hits = match event {
            CurveCurveIntersectionEvent::Point(hit) => [Some(hit), None],
            CurveCurveIntersectionEvent::Overlap(span) => [Some(span.start()), Some(span.end())],
        };
        for hit in hits.into_iter().flatten() {
            let score = picks[0].distance_to(hit.point())? + picks[1].distance_to(hit.point())?;
            if best.as_ref().is_none_or(|(old, _)| score < *old) {
                best = Some((score, hit.point()));
            }
        }
    }
    let (_, intersection) = best.ok_or(CommandError::Usage(
        "No curve intersection satisfies these picks",
    ))?;
    let mut parameters = [0.; 2];
    let mut points = [intersection; 2];
    for i in 0..2 {
        let corner = support::parameter(curves[i], intersection, tolerance)?;
        let domain = curves[i].as_ref().domain();
        let width = *domain.end() - *domain.start();
        let forward = if closed[i] {
            (seeds[i] - corner).rem_euclid(width) < width * 0.5
        } else {
            seeds[i] > corner
        };
        if options.distances[i] == 0. {
            parameters[i] = corner;
            continue;
        }
        let search = if closed[i] {
            crate::curve_segments::seam(curves[i], corner)?
        } else {
            support::retained(curves[i], corner, intersection, forward)?
        };
        let station = search
            .as_ref()
            .point_at_chord_distance(options.distances[i], !forward, tolerance)?
            .ok_or(CommandError::Usage("Chamfer distance consumes the source"))?;
        parameters[i] = support::parameter(curves[i], station.point, tolerance)?;
        points[i] = station.point;
    }
    let insertion = if points[0].distance_to(points[1])? > tolerance.absolute() {
        Some(Curve3::Line(LineSegment::try_new(
            points[0],
            points[1],
            Tolerance::NUMERICAL_VALIDATION,
        )?))
    } else {
        None
    };
    closed::prepare(
        curves,
        seeds,
        closed,
        Contacts {
            parameters,
            points,
            insertion,
        },
        options.trim,
        options.join,
        tolerance,
    )
}
