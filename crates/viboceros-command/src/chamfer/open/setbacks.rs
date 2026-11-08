use super::*;
use crate::curve_pair_edit::closed::Contacts;
mod overflow;
mod smooth;

pub(super) struct Solution {
    pub contacts: Contacts,
    pub consumed: [bool; 2],
    pub invalid_join: bool,
    pub reset_arc_domains: [bool; 2],
}

pub(super) fn solve(
    input: &mut contacts::Solution,
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Solution, CommandError> {
    if input.smooth_overlap && options.distances != [0., 0.] {
        return smooth::solve(input, options, tolerance);
    }
    let mut parameters = input.corners;
    let mut points = input.points;
    let mut extensions: [Option<overflow::Continuation>; 2] = [None, None];
    let retain_end = std::array::from_fn(|i| {
        input.seeds[i] > input.corners[i]
            || input.corners[i] == *input.curves[i].as_ref().domain().start()
    });
    for i in 0..2 {
        let curve = &input.curves[i];
        if options.distances[i] == 0. {
            continue;
        }
        if let Curve3::Line(line) = curve {
            let d = line.domain();
            parameters[i] += if retain_end[i] { 1. } else { -1. } * options.distances[i]
                / line.length()?
                * (*d.end() - *d.start());
            points[i] = line.point_at((parameters[i] - *d.start()) / (*d.end() - *d.start()))?;
        } else {
            let search = support::retained(curve, input.corners[i], points[i], retain_end[i])?;
            if let Some(station) = search.as_ref().point_at_chord_distance(
                options.distances[i],
                !retain_end[i],
                tolerance,
            )? {
                parameters[i] = support::parameter(curve, station.point, tolerance)?;
                points[i] = station.point;
            } else {
                let extension = overflow::solve(
                    curve,
                    input.corners[i],
                    input.points[i],
                    retain_end[i],
                    options.distances[i],
                    options,
                )?;
                points[i] = extension.cut;
                extensions[i] = Some(extension);
            }
        }
    }
    let insertion = if options.distances != [0., 0.]
        && points[0].distance_to(points[1])? > tolerance.absolute()
    {
        Some(Curve3::Line(LineSegment::try_new(
            points[0],
            points[1],
            Tolerance::NUMERICAL_VALIDATION,
        )?))
    } else {
        None
    };
    let cut_points = points;
    let mut reset_arc_domains = [false; 2];
    let mut consumed = [false; 2];
    let mut invalid_join = false;
    for i in 0..2 {
        if let Some(extension) = extensions[i].take() {
            input.curves[i] = extension.curve;
            reset_arc_domains[i] = extension.reset_arc_domain;
            let curve = input.curves[i].as_ref();
            let d = curve.domain();
            parameters[i] = if retain_end[i] { *d.start() } else { *d.end() };
            points[i] = curve.evaluate(parameters[i])?;
            invalid_join |= points[i].distance_to(cut_points[i])? > tolerance.absolute();
            continue;
        }
        let curve = input.curves[i].as_ref();
        let d = curve.domain();
        let far = if retain_end[i] { *d.end() } else { *d.start() };
        if parameters[i] == far {
            consumed[i] = true;
            if !options.join {
                parameters[i] = if retain_end[i] { *d.start() } else { *d.end() };
                points[i] = curve.evaluate(parameters[i])?;
            }
        } else if let Curve3::Line(line) = &input.curves[i]
            && ((retain_end[i] && parameters[i] > far) || (!retain_end[i] && parameters[i] < far))
        {
            parameters[i] = 2. * if retain_end[i] { *d.start() } else { *d.end() } - parameters[i];
            points[i] = line.point_at((parameters[i] - *d.start()) / (*d.end() - *d.start()))?;
            invalid_join = true;
        }
    }
    Ok(Solution {
        contacts: Contacts {
            parameters,
            points,
            insertion,
            reset_arc_domains: [false; 2],
            retain_end: Some(retain_end),
        },
        consumed,
        invalid_join,
        reset_arc_domains,
    })
}
