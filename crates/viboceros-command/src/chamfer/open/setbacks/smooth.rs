//! Smooth collinear replacement supports and bevel supports are distinct.
use super::*;
use crate::curve_pair_edit::{line_contacts, line_supports};
use viboceros_geometry::BoundingBox3;

pub(super) fn solve(
    input: &contacts::Solution,
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Solution, CommandError> {
    let curves = [&input.curves[0], &input.curves[1]];
    let [Curve3::Line(a), Curve3::Line(b)] = curves else {
        unreachable!("line overlap")
    };
    let bounds = BoundingBox3::from_points([a.start(), a.end(), b.start(), b.end()])?;
    let padding = 50. * bounds.min().distance_to(bounds.max())?;
    let supports =
        line_supports::from_fractions(curves, [padding / a.length()?, padding / b.length()?])?
            .expect("two line supports");
    let bevel = line_contacts::from_supports(
        curves,
        input.seeds,
        &supports,
        CurveOtherExtensionStyle::Line,
        false,
        tolerance,
    )?
    .contacts;
    let mut points = bevel.points;
    for (i, point) in points.iter_mut().enumerate() {
        let line = if i == 0 { a } else { b };
        let forward = input.seeds[i] > bevel.parameters[i];
        *point =
            point.translated(line.direction(tolerance)?.as_vector().scaled(if forward {
                options.distances[i]
            } else {
                -options.distances[i]
            })?)?;
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
    let invalid_join = input.points[0].distance_to(points[0])? > tolerance.absolute()
        || input.points[1].distance_to(points[1])? > tolerance.absolute();
    Ok(Solution {
        contacts: Contacts {
            parameters: input.corners,
            points: input.points,
            insertion,
            reset_arc_domains: [false; 2],
            retain_end: Some(std::array::from_fn(|i| input.seeds[i] > input.corners[i])),
        },
        consumed: [false; 2],
        invalid_join,
        reset_arc_domains: [false; 2],
    })
}
