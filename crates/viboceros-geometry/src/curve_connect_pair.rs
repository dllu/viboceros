//! Endpoint connections using exact lines and straight tangent extensions.

use crate::{
    Curve3, CurveSegment3, GeometryError, LineSegment, ParameterSide, Point3, PolyCurve3,
    Tolerance, UnitVector3,
    curve_pair_support::{
        curve_from_segments, oriented, original_direction, selected_end,
        supporting_directions_intersection,
    },
};

/// How a nonmeeting circular arc endpoint is extended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveArcExtensionStyle {
    Arc,
    Line,
}

/// Connects selected ends while retaining each source's original direction.
/// Lines may be trimmed or extended. NURBS and polyline ends may extend along
/// their exact endpoint tangents; arc extension requires a separate style.
pub fn try_connect_curves_parts(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    try_connect_curves_parts_with_arc_style(
        first,
        first_pick,
        second,
        second_pick,
        CurveArcExtensionStyle::Arc,
        tolerance,
    )
}

/// Connects selected ends with a chosen arc-extension style.
pub fn try_connect_curves_parts_with_arc_style(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    arc_extension: CurveArcExtensionStyle,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    let reverse_first = !selected_end(first, first_pick, tolerance)?;
    let reverse_second = selected_end(second, second_pick, tolerance)?;
    let (first, second) = connected_oriented(
        first,
        first_pick,
        second,
        second_pick,
        arc_extension,
        tolerance,
    )?;
    Ok(vec![
        original_direction(
            curve_from_segments(first.segments())?,
            reverse_first,
            tolerance,
        )?,
        original_direction(
            curve_from_segments(second.segments())?,
            reverse_second,
            tolerance,
        )?,
    ])
}

/// Connects and joins the selected ends into one native polycurve.
pub fn try_connect_curves_joined(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    try_connect_curves_joined_with_arc_style(
        first,
        first_pick,
        second,
        second_pick,
        CurveArcExtensionStyle::Arc,
        tolerance,
    )
}

/// Connects and joins using a chosen arc-extension style.
pub fn try_connect_curves_joined_with_arc_style(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    arc_extension: CurveArcExtensionStyle,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    let (first, second) = connected_oriented(
        first,
        first_pick,
        second,
        second_pick,
        arc_extension,
        tolerance,
    )?;
    let mut segments = first.segments().to_vec();
    segments.extend_from_slice(second.segments());
    PolyCurve3::try_new(segments)
}

fn connected_oriented(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    arc_extension: CurveArcExtensionStyle,
    tolerance: Tolerance,
) -> Result<(PolyCurve3, PolyCurve3), GeometryError> {
    let first = oriented(first, first_pick, true, tolerance)?;
    let second = oriented(second, second_pick, false, tolerance)?;
    let last = first.segments().len() - 1;
    let tail = &first.segments()[last];
    let head = &second.segments()[0];
    let tail_end = tail.evaluate(*tail.domain().end())?;
    let head_start = head.evaluate(*head.domain().start())?;
    if tail_end.distance_to(head_start)? <= tolerance.absolute() {
        return Ok((first, second));
    }
    let (first_origin, first_direction) = support_direction(tail, true, arc_extension, tolerance)?;
    let (second_origin, second_direction) =
        support_direction(head, false, arc_extension, tolerance)?;
    let meeting = supporting_directions_intersection(
        first_origin,
        first_direction,
        second_origin,
        second_direction,
        tolerance,
    )?;
    let first_retained = retained_tail(tail, meeting, tolerance)?;
    let second_retained = retained_head(head, meeting, tolerance)?;
    let mut first_segments = first.segments()[..last].to_vec();
    first_segments.extend(first_retained);
    let mut second_segments = second_retained;
    second_segments.extend_from_slice(&second.segments()[1..]);
    let first = PolyCurve3::try_new(first_segments)?;
    let second = PolyCurve3::try_new(second_segments)?;
    if first
        .evaluate(*first.domain().end())?
        .distance_to(second.evaluate(*second.domain().start())?)?
        > tolerance.absolute()
    {
        return Err(unsupported());
    }
    Ok((first, second))
}

fn support_direction(
    terminal: &CurveSegment3,
    at_end: bool,
    arc_extension: CurveArcExtensionStyle,
    tolerance: Tolerance,
) -> Result<(Point3, UnitVector3), GeometryError> {
    if let CurveSegment3::Line(line) = terminal {
        return Ok((line.start(), line.direction(tolerance)?));
    }
    if !(matches!(
        terminal,
        CurveSegment3::NurbsCurve(_) | CurveSegment3::Polyline(_)
    ) || matches!(terminal, CurveSegment3::Arc(_))
        && arc_extension == CurveArcExtensionStyle::Line)
    {
        return Err(unsupported());
    }
    let parameter = if at_end {
        *terminal.domain().end()
    } else {
        *terminal.domain().start()
    };
    let side = if at_end {
        ParameterSide::Left
    } else {
        ParameterSide::Right
    };
    let sample = terminal
        .as_ref()
        .evaluate_with_tangent_on_side(parameter, side)?;
    Ok((sample.point(), sample.tangent()))
}

fn retained_tail(
    terminal: &CurveSegment3,
    meeting: Point3,
    tolerance: Tolerance,
) -> Result<Vec<CurveSegment3>, GeometryError> {
    if let CurveSegment3::Line(line) = terminal {
        let direction = line.direction(tolerance)?.as_vector();
        if line.start().vector_to(meeting)?.dot(direction)? <= tolerance.absolute() {
            return Err(unsupported());
        }
        return Ok(vec![CurveSegment3::Line(LineSegment::try_new(
            line.start(),
            meeting,
            tolerance,
        )?)]);
    }
    let endpoint = terminal.evaluate(*terminal.domain().end())?;
    let direction = terminal
        .as_ref()
        .evaluate_with_tangent_on_side(*terminal.domain().end(), ParameterSide::Left)?
        .tangent()
        .as_vector();
    let forward = endpoint.vector_to(meeting)?.dot(direction)?;
    if forward < -tolerance.absolute() {
        return Err(unsupported());
    }
    let mut segments = vec![terminal.clone()];
    if endpoint.distance_to(meeting)? > tolerance.absolute() {
        segments.push(CurveSegment3::Line(LineSegment::try_new(
            endpoint,
            meeting,
            Tolerance::NUMERICAL_VALIDATION,
        )?));
    }
    Ok(segments)
}

fn retained_head(
    terminal: &CurveSegment3,
    meeting: Point3,
    tolerance: Tolerance,
) -> Result<Vec<CurveSegment3>, GeometryError> {
    if let CurveSegment3::Line(line) = terminal {
        let direction = line.direction(tolerance)?.as_vector();
        if line.start().vector_to(meeting)?.dot(direction)? >= line.length()? - tolerance.absolute()
        {
            return Err(unsupported());
        }
        return Ok(vec![CurveSegment3::Line(LineSegment::try_new(
            meeting,
            line.end(),
            tolerance,
        )?)]);
    }
    let endpoint = terminal.evaluate(*terminal.domain().start())?;
    let direction = terminal
        .as_ref()
        .evaluate_with_tangent_on_side(*terminal.domain().start(), ParameterSide::Right)?
        .tangent()
        .as_vector();
    let forward = endpoint.vector_to(meeting)?.dot(direction)?;
    if forward > tolerance.absolute() {
        return Err(unsupported());
    }
    let mut segments = Vec::new();
    if endpoint.distance_to(meeting)? > tolerance.absolute() {
        segments.push(CurveSegment3::Line(LineSegment::try_new(
            meeting,
            endpoint,
            Tolerance::NUMERICAL_VALIDATION,
        )?));
    }
    segments.push(terminal.clone());
    Ok(segments)
}

fn unsupported() -> GeometryError {
    GeometryError::InvalidPolyCurve {
        context: "selected curve ends cannot form a supported connection",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CircularArc3, NurbsCurve, Real};

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }

    fn line(a: Point3, b: Point3) -> Curve3 {
        Curve3::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
    }

    fn curve(a: Point3, b: Point3, c: Point3) -> Curve3 {
        Curve3::NurbsCurve(
            NurbsCurve::try_new(2, vec![a, b, c], vec![0., 0., 0., 1., 1., 1.]).unwrap(),
        )
    }

    #[test]
    fn extends_a_nurbs_tangent_to_meet_a_line() {
        let first = curve(p(0., 0.), p(1., 0.), p(2., 1.));
        let second = line(p(3., 3.), p(3., 4.));
        let joined =
            try_connect_curves_joined(&first, p(2., 1.), &second, p(3., 3.), Tolerance::DEFAULT)
                .unwrap();
        assert!(matches!(
            joined.segments(),
            [
                CurveSegment3::NurbsCurve(_),
                CurveSegment3::Line(_),
                CurveSegment3::Line(_)
            ]
        ));
        let CurveSegment3::Line(extension) = &joined.segments()[1] else {
            unreachable!()
        };
        assert!(extension.end().distance_to(p(3., 2.)).unwrap() < 1e-12);
        let CurveSegment3::Line(retained) = &joined.segments()[2] else {
            unreachable!()
        };
        assert!(retained.start().distance_to(p(3., 2.)).unwrap() < 1e-12);
    }

    #[test]
    fn separate_curves_keep_original_directions() {
        let first = curve(p(2., 1.), p(1., 0.), p(0., 0.));
        let second = line(p(3., 4.), p(3., 3.));
        let parts =
            try_connect_curves_parts(&first, p(2., 1.), &second, p(3., 3.), Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(parts.len(), 2);
        let first_ref = parts[0].as_ref();
        let second_ref = parts[1].as_ref();
        assert!(
            first_ref
                .start_point()
                .unwrap()
                .distance_to(p(3., 2.))
                .unwrap()
                < 1e-12
        );
        assert!(
            second_ref
                .end_point()
                .unwrap()
                .distance_to(p(3., 2.))
                .unwrap()
                < 1e-12
        );
    }

    #[test]
    fn tangent_support_survives_large_world_coordinates() {
        let base = 1.0e16;
        let first = curve(
            p(base, base),
            p(base + 4096., base),
            p(base + 8192., base + 6144.),
        );
        let second = line(
            p(base + 12288., base + 16384.),
            p(base + 12288., base + 20480.),
        );
        let joined = try_connect_curves_joined(
            &first,
            p(base + 8192., base + 6144.),
            &second,
            p(base + 12288., base + 16384.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let CurveSegment3::Line(extension) = &joined.segments()[1] else {
            panic!("tangent extension")
        };
        assert!(
            extension
                .end()
                .distance_to(p(base + 12288., base + 12288.))
                .unwrap()
                < 10.0
        );
    }

    #[test]
    fn two_nurbs_ends_meet_through_tangent_extensions() {
        let first = curve(p(0., 0.), p(1., 0.), p(2., 0.));
        let second = curve(p(3., 1.), p(3., 2.), p(3., 3.));
        let joined =
            try_connect_curves_joined(&first, p(2., 0.), &second, p(3., 1.), Tolerance::DEFAULT)
                .unwrap();
        assert!(matches!(
            joined.segments(),
            [
                CurveSegment3::NurbsCurve(_),
                CurveSegment3::Line(_),
                CurveSegment3::Line(_),
                CurveSegment3::NurbsCurve(_)
            ]
        ));
        let CurveSegment3::Line(first_extension) = &joined.segments()[1] else {
            unreachable!()
        };
        let CurveSegment3::Line(second_extension) = &joined.segments()[2] else {
            unreachable!()
        };
        assert!(first_extension.end().distance_to(p(3., 0.)).unwrap() < 1e-12);
        assert!(second_extension.start().distance_to(p(3., 0.)).unwrap() < 1e-12);
    }

    #[test]
    fn arc_line_style_adds_tangent_extension() {
        let diagonal = 2.0_f64.sqrt() / 2.0;
        let arc = Curve3::Arc(
            CircularArc3::try_from_three_points(
                p(1., 0.),
                p(diagonal, diagonal),
                p(0., 1.),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        let second = line(p(-1., 2.), p(-1., 3.));
        assert!(
            try_connect_curves_joined(&arc, p(0., 1.), &second, p(-1., 2.), Tolerance::DEFAULT)
                .is_err()
        );
        let joined = try_connect_curves_joined_with_arc_style(
            &arc,
            p(0., 1.),
            &second,
            p(-1., 2.),
            CurveArcExtensionStyle::Line,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(matches!(
            joined.segments(),
            [
                CurveSegment3::Arc(_),
                CurveSegment3::Line(_),
                CurveSegment3::Line(_)
            ]
        ));
        let CurveSegment3::Line(extension) = &joined.segments()[1] else {
            unreachable!()
        };
        assert!(extension.end().distance_to(p(-1., 1.)).unwrap() < 1e-12);
    }
}
