//! Joined, trimmed fillets between selected ends of two open curves.

use crate::{
    Curve3, CurveSegment3, GeometryError, LineSegment, Point3, PolyCurve3, Real, Tolerance,
};

/// Trims or extends selected curve ends to a tangent circular fillet and joins
/// the two retained curves with that arc. Zero radius joins at a sharp corner.
/// Pick points choose which end of each curve participates. Noncoincident
/// terminal lines may meet by extension.
pub fn try_fillet_curves_joined(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    if !radius.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "curve fillet radius",
        });
    }
    if radius < 0.0 || (radius > 0.0 && radius <= tolerance.absolute()) {
        return Err(GeometryError::Degenerate {
            context: "curve fillet radius",
        });
    }
    let first = oriented(first, first_pick, true, tolerance)?;
    let second = oriented(second, second_pick, false, tolerance)?;
    let last = first.segments().len() - 1;
    let first_terminal = first.segments()[last].clone();
    let second_terminal = second.segments()[0].clone();
    let (first_terminal, second_terminal) =
        if let (CurveSegment3::Line(first_line), CurveSegment3::Line(second_line)) =
            (&first_terminal, &second_terminal)
        {
            let (a, b) = meeting_lines(*first_line, *second_line, tolerance)?;
            (CurveSegment3::Line(a), CurveSegment3::Line(b))
        } else {
            if first_terminal
                .as_ref()
                .evaluate(*first_terminal.domain().end())?
                .distance_to(
                    second_terminal
                        .as_ref()
                        .evaluate(*second_terminal.domain().start())?,
                )?
                > tolerance.absolute()
            {
                return Err(unsupported());
            }
            (first_terminal, second_terminal)
        };
    let mut segments = first.segments()[..last].to_vec();
    if radius == 0.0 {
        segments.extend([first_terminal, second_terminal]);
    } else {
        let pair = PolyCurve3::try_new(vec![first_terminal, second_terminal])?;
        let rounded = pair.try_fillet_corners(radius, tolerance)?;
        if rounded.segments().len() != 3 || !matches!(rounded.segments()[1], CurveSegment3::Arc(_))
        {
            return Err(unsupported());
        }
        segments.extend_from_slice(rounded.segments());
    }
    segments.extend_from_slice(&second.segments()[1..]);
    PolyCurve3::try_new(segments)
}

fn oriented(
    source: &Curve3,
    pick: Point3,
    pick_at_end: bool,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    if source.as_ref().is_closed()? {
        return Err(unsupported());
    }
    let start = source
        .as_ref()
        .evaluate(*source.as_ref().domain().start())?;
    let end = source.as_ref().evaluate(*source.as_ref().domain().end())?;
    let start_distance = pick.distance_to(start)?;
    let end_distance = pick.distance_to(end)?;
    if (start_distance - end_distance).abs() <= tolerance.absolute() {
        return Err(GeometryError::InvalidPolyCurve {
            context: "curve fillet pick does not identify an end",
        });
    }
    let selected_end = end_distance < start_distance;
    if selected_end == pick_at_end {
        source.to_polycurve()
    } else {
        source.reversed(tolerance)?.to_polycurve()
    }
}

fn meeting_lines(
    first: LineSegment,
    second: LineSegment,
    tolerance: Tolerance,
) -> Result<(LineSegment, LineSegment), GeometryError> {
    let first_direction = first.direction(tolerance)?.as_vector();
    let second_direction = second.direction(tolerance)?.as_vector();
    let normal = first_direction.cross(second_direction)?;
    let denominator = normal.dot(normal)?;
    if denominator <= tolerance.angular().sin().powi(2) {
        return Err(unsupported());
    }
    let between = first.start().vector_to(second.start())?;
    let first_distance = between.cross(second_direction)?.dot(normal)? / denominator;
    let second_distance = between.cross(first_direction)?.dot(normal)? / denominator;
    let first_meeting = first
        .start()
        .translated(first_direction.scaled(first_distance)?)?;
    let second_meeting = second
        .start()
        .translated(second_direction.scaled(second_distance)?)?;
    if first_meeting.distance_to(second_meeting)? > tolerance.absolute()
        || first_distance <= tolerance.absolute()
        || second_distance >= second.length()? - tolerance.absolute()
    {
        return Err(unsupported());
    }
    let meeting = first_meeting.midpoint(second_meeting)?;
    Ok((
        LineSegment::try_new(first.start(), meeting, Tolerance::NUMERICAL_VALIDATION)?,
        LineSegment::try_new(meeting, second.end(), Tolerance::NUMERICAL_VALIDATION)?,
    ))
}

fn unsupported() -> GeometryError {
    GeometryError::InvalidPolyCurve {
        context: "selected curve ends cannot form a supported fillet",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }

    fn line(a: Point3, b: Point3) -> Curve3 {
        Curve3::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
    }

    #[test]
    fn joined_fillet_handles_meeting_and_extended_lines() {
        for (first_end, second_start) in [(p(4., 0.), p(4., 0.)), (p(3., 0.), p(4., 1.))] {
            let result = try_fillet_curves_joined(
                &line(p(0., 0.), first_end),
                first_end,
                &line(second_start, p(4., 4.)),
                second_start,
                0.5,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(result.segments().len(), 3);
            assert!(matches!(result.segments()[1], CurveSegment3::Arc(_)));
            assert!(!result.is_closed().unwrap());
            assert!(
                result
                    .evaluate(*result.domain().start())
                    .unwrap()
                    .distance_to(p(0., 0.))
                    .unwrap()
                    < 1e-12
            );
            assert!(
                result
                    .evaluate(*result.domain().end())
                    .unwrap()
                    .distance_to(p(4., 4.))
                    .unwrap()
                    < 1e-12
            );
        }
    }

    #[test]
    fn earlier_polycurve_corner_is_not_filleted() {
        let first = Curve3::PolyCurve(
            PolyCurve3::try_new(vec![
                CurveSegment3::Line(
                    LineSegment::try_new(p(0., 0.), p(0., 2.), Tolerance::DEFAULT).unwrap(),
                ),
                CurveSegment3::Line(
                    LineSegment::try_new(p(0., 2.), p(4., 2.), Tolerance::DEFAULT).unwrap(),
                ),
            ])
            .unwrap(),
        );
        let result = try_fillet_curves_joined(
            &first,
            p(4., 2.),
            &line(p(4., 2.), p(4., 6.)),
            p(4., 2.),
            0.5,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result.segments().len(), 4);
        assert!(matches!(result.segments()[0], CurveSegment3::Line(_)));
        assert!(matches!(result.segments()[1], CurveSegment3::Line(_)));
        assert!(matches!(result.segments()[2], CurveSegment3::Arc(_)));
        assert!(matches!(result.segments()[3], CurveSegment3::Line(_)));
    }

    #[test]
    fn zero_radius_joins_at_the_intersection_without_an_arc() {
        let result = try_fillet_curves_joined(
            &line(p(0., 0.), p(3., 0.)),
            p(3., 0.),
            &line(p(4., 1.), p(4., 4.)),
            p(4., 1.),
            0.0,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result.segments().len(), 2);
        assert!(
            result
                .segments()
                .iter()
                .all(|part| matches!(part, CurveSegment3::Line(_)))
        );
        assert!((result.length(Tolerance::DEFAULT).unwrap() - 8.0).abs() < 1e-12);
    }
}
