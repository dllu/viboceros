//! Intersection-first setbacks keep every closed source in full.
use super::*;
use crate::curve_pair_edit::{
    Plan,
    closed::{self, Contacts},
};
use viboceros_geometry::{Curve3, CurveCurveIntersectionEvent, LineSegment};
mod extension;
mod support;

pub(super) fn prepare(
    curves: [&Curve3; 2],
    picks: [Point3; 2],
    closed: [bool; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    let original_domains = curves.map(|c| c.as_ref().domain());
    let mut working = curves.map(Clone::clone);
    let mut reset_arc_domains = [false; 2];
    let intersections = |curves: [&Curve3; 2]| -> Result<_, CommandError> {
        let supports = [
            support::curve(curves[0], curves[1], tolerance)?,
            support::curve(curves[1], curves[0], tolerance)?,
        ];
        crate::curve_intersections::events(
            &supports[0].as_ref().to_nurbs()?,
            &supports[1].as_ref().to_nurbs()?,
            tolerance,
        )
    };
    let mut events = intersections([&working[0], &working[1]])?;
    let mut extended = false;
    if events.is_empty() {
        for i in 0..2 {
            if !closed[i] && !matches!(curves[i], Curve3::Line(_)) {
                working[i] =
                    extension::to_boundary(curves[i], curves[1 - i], picks[i], options, tolerance)?;
                reset_arc_domains[i] = matches!(&working[i], Curve3::Arc(_));
                extended = true;
            }
        }
        if extended {
            events = intersections([&working[0], &working[1]])?;
        }
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
    let mut retain_end = [false; 2];
    for i in 0..2 {
        let corner = support::parameter(curves[i], intersection, tolerance)?;
        let domain = curves[i].as_ref().domain();
        let width = *domain.end() - *domain.start();
        let forward = if closed[i] {
            (seeds[i] - corner).rem_euclid(width) < width * 0.5
        } else {
            seeds[i] > corner
        };
        retain_end[i] = forward;
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
    for i in 0..2 {
        reset_arc_domains[i] &= !original_domains[i].contains(&parameters[i]);
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
            reset_arc_domains,
            retain_end: Some(retain_end),
        },
        options.trim,
        options.join,
        tolerance,
    )
}
