//! Joined, trimmed fillets between selected ends of two open curves.

use crate::{
    Curve3, CurveSegment3, GeometryError, Point3, PolyCurve3, Real, Tolerance,
    curve_pair_support::{
        curve_from_segments, meeting_lines, oriented, original_direction, selected_end, unsupported,
    },
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

/// Returns Rhino's separate fillet pieces. With trimming enabled, the
/// retained first and second curves precede the circular fillet. Without
/// trimming, only the fillet arc is returned and the inputs stay untouched.
pub fn try_fillet_curves_parts(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    radius: Real,
    trim: bool,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    if !trim && radius == 0.0 {
        return Err(unsupported());
    }
    let reverse_first = !selected_end(first, first_pick, tolerance)?;
    let reverse_second = selected_end(second, second_pick, tolerance)?;
    let first_count = first.to_polycurve()?.segments().len();
    let joined =
        try_fillet_curves_joined(first, first_pick, second, second_pick, radius, tolerance)?;
    let segments = joined.segments();
    if radius == 0.0 {
        return Ok(vec![
            original_direction(
                curve_from_segments(&segments[..first_count])?,
                reverse_first,
                tolerance,
            )?,
            original_direction(
                curve_from_segments(&segments[first_count..])?,
                reverse_second,
                tolerance,
            )?,
        ]);
    }
    let arc = match &segments[first_count] {
        CurveSegment3::Arc(arc) => Curve3::Arc(*arc),
        _ => return Err(unsupported()),
    };
    if !trim {
        return Ok(vec![arc]);
    }
    Ok(vec![
        original_direction(
            curve_from_segments(&segments[..first_count])?,
            reverse_first,
            tolerance,
        )?,
        original_direction(
            curve_from_segments(&segments[first_count + 1..])?,
            reverse_second,
            tolerance,
        )?,
        arc,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineSegment;

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
