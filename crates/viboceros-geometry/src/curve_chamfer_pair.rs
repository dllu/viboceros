//! Straight chamfers between selected ends of open curves.

use crate::{
    Curve3, CurveSegment3, GeometryError, LineSegment, Point3, PolyCurve3, Real, Tolerance,
    curve_fillet_pair::{
        curve_from_segments, meeting_lines, oriented, original_direction, selected_end,
    },
};

/// Trims or extends terminal straight segments to a bevel whose setbacks are
/// measured along the two supporting lines from their intersection.
pub fn try_chamfer_curves_joined(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    first_distance: Real,
    second_distance: Real,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    validate_distances(first_distance, second_distance)?;
    let first = oriented(first, first_pick, true, tolerance)?;
    let second = oriented(second, second_pick, false, tolerance)?;
    let last = first.segments().len() - 1;
    let (CurveSegment3::Line(first_terminal), CurveSegment3::Line(second_terminal)) =
        (&first.segments()[last], &second.segments()[0])
    else {
        return Err(unsupported());
    };
    let (first_line, second_line) = meeting_lines(*first_terminal, *second_terminal, tolerance)?;
    let corner = first_line.end();
    let first_direction = first_line.direction(tolerance)?.as_vector();
    let second_direction = second_line.direction(tolerance)?.as_vector();
    let first_cut = corner.translated(first_direction.scaled(-first_distance)?)?;
    let second_cut = corner.translated(second_direction.scaled(second_distance)?)?;
    if first_cut.distance_to(first_line.start())? <= tolerance.absolute()
        || second_cut.distance_to(second_line.end())? <= tolerance.absolute()
        || first_distance > first_line.length()? - tolerance.absolute()
        || second_distance > second_line.length()? - tolerance.absolute()
    {
        return Err(unsupported());
    }
    let mut segments = first.segments()[..last].to_vec();
    segments.push(CurveSegment3::Line(LineSegment::try_new(
        first_line.start(),
        first_cut,
        Tolerance::NUMERICAL_VALIDATION,
    )?));
    if first_cut.distance_to(second_cut)? > tolerance.absolute() {
        segments.push(CurveSegment3::Line(LineSegment::try_new(
            first_cut,
            second_cut,
            Tolerance::NUMERICAL_VALIDATION,
        )?));
    }
    segments.push(CurveSegment3::Line(LineSegment::try_new(
        second_cut,
        second_line.end(),
        Tolerance::NUMERICAL_VALIDATION,
    )?));
    segments.extend_from_slice(&second.segments()[1..]);
    PolyCurve3::try_new(segments)
}

/// Returns retained source curves in their original directions, followed by
/// the bevel. With trimming off, only the bevel is returned.
pub fn try_chamfer_curves_parts(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    distances: [Real; 2],
    trim: bool,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    if !trim && distances == [0.0, 0.0] {
        return Err(unsupported());
    }
    let reverse_first = !selected_end(first, first_pick, tolerance)?;
    let reverse_second = selected_end(second, second_pick, tolerance)?;
    let first_count = first.to_polycurve()?.segments().len();
    let joined = try_chamfer_curves_joined(
        first,
        first_pick,
        second,
        second_pick,
        distances[0],
        distances[1],
        tolerance,
    )?;
    let segments = joined.segments();
    if !trim {
        return Ok(vec![segments[first_count].clone().into_curve()]);
    }
    let second_start = if distances == [0.0, 0.0] {
        first_count
    } else {
        first_count + 1
    };
    let mut parts = vec![
        original_direction(
            curve_from_segments(&segments[..first_count])?,
            reverse_first,
            tolerance,
        )?,
        original_direction(
            curve_from_segments(&segments[second_start..])?,
            reverse_second,
            tolerance,
        )?,
    ];
    if second_start > first_count {
        parts.push(segments[first_count].clone().into_curve());
    }
    Ok(parts)
}

fn validate_distances(first: Real, second: Real) -> Result<(), GeometryError> {
    if !first.is_finite() || !second.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "curve chamfer distance",
        });
    }
    if first < 0.0 || second < 0.0 {
        return Err(GeometryError::Degenerate {
            context: "curve chamfer distance",
        });
    }
    Ok(())
}

fn unsupported() -> GeometryError {
    GeometryError::InvalidPolyCurve {
        context: "selected curve ends cannot form a supported chamfer",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn line(a: Point3, b: Point3) -> Curve3 {
        Curve3::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
    }

    #[test]
    fn unequal_setbacks_and_extended_lines() {
        let joined = try_chamfer_curves_joined(
            &line(p(0., 0.), p(3., 0.)),
            p(2.9, 0.),
            &line(p(4., 1.), p(4., 4.)),
            p(4., 1.1),
            1.,
            0.5,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(joined.segments().len(), 3);
        let points = joined
            .segments()
            .iter()
            .map(|segment| {
                let CurveSegment3::Line(line) = segment else {
                    panic!("straight chamfer")
                };
                [line.start(), line.end()]
            })
            .collect::<Vec<_>>();
        assert_eq!(
            points,
            vec![
                [p(0., 0.), p(3., 0.)],
                [p(3., 0.), p(4., 0.5)],
                [p(4., 0.5), p(4., 4.)],
            ]
        );
    }

    #[test]
    fn separate_pieces_preserve_source_directions() {
        let parts = try_chamfer_curves_parts(
            &line(p(4., 0.), p(0., 0.)),
            p(3.8, 0.),
            &line(p(4., 4.), p(4., 0.)),
            p(4., 0.2),
            [0.5, 1.],
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [
            Curve3::Line(first),
            Curve3::Line(second),
            Curve3::Line(bevel),
        ] = parts.as_slice()
        else {
            panic!("three straight pieces");
        };
        assert_eq!((first.start(), first.end()), (p(3.5, 0.), p(0., 0.)));
        assert_eq!((second.start(), second.end()), (p(4., 4.), p(4., 1.)));
        assert_eq!((bevel.start(), bevel.end()), (p(3.5, 0.), p(4., 1.)));
    }

    #[test]
    fn oblique_setbacks_follow_each_curve_length() {
        let joined = try_chamfer_curves_joined(
            &line(p(0., 0.), p(4., 0.)),
            p(3.8, 0.),
            &line(p(4., 0.), p(6., 2.)),
            p(4.2, 0.2),
            1.,
            1.,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let CurveSegment3::Line(bevel) = &joined.segments()[1] else {
            panic!("straight bevel");
        };
        assert!(bevel.start().distance_to(p(3., 0.)).unwrap() < 1e-12);
        let diagonal = 2.0_f64.sqrt() / 2.0;
        assert!(bevel.end().distance_to(p(4. + diagonal, diagonal)).unwrap() < 1e-12);
    }

    #[test]
    fn skew_lines_have_no_chamfer() {
        let raised = |x: Real, y: Real| Point3::try_new(x, y, 1.).unwrap();
        assert!(
            try_chamfer_curves_joined(
                &line(p(0., 0.), p(4., 0.)),
                p(3.8, 0.),
                &line(raised(4., 0.), raised(4., 4.)),
                raised(4., 0.2),
                0.5,
                0.5,
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }
}
