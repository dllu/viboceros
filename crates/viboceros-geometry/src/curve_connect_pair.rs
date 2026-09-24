//! Endpoint connections using exact lines, circular supports, and tangents.

use crate::{
    CircularArc3, Curve3, CurveSegment3, GeometryError, LineSegment, ParameterSide, Point3,
    PolyCurve3, Real, Tolerance, UnitVector3, Vector3,
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
/// their exact endpoint tangents. Coplanar arc/line pairs may extend the arc.
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
    if arc_extension == CurveArcExtensionStyle::Arc {
        let arc_line = match (tail, head) {
            (CurveSegment3::Arc(arc), CurveSegment3::Line(line)) => Some(connect_arc_line(
                &first, &second, *arc, *line, true, tolerance,
            )?),
            (CurveSegment3::Line(line), CurveSegment3::Arc(arc)) => Some(connect_arc_line(
                &first, &second, *arc, *line, false, tolerance,
            )?),
            (CurveSegment3::Arc(before), CurveSegment3::Arc(after)) => Some(connect_arc_arc(
                &first, &second, *before, *after, tolerance,
            )?),
            _ => None,
        };
        if let Some(connected) = arc_line {
            return Ok(connected);
        }
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

fn connect_arc_arc(
    first: &PolyCurve3,
    second: &PolyCurve3,
    before: CircularArc3,
    after: CircularArc3,
    tolerance: Tolerance,
) -> Result<(PolyCurve3, PolyCurve3), GeometryError> {
    let normal = before.normal()?.as_vector();
    if normal.cross(after.normal()?.as_vector())?.length()? > tolerance.angular().sin() {
        return Err(unsupported());
    }
    let centers = before.center().vector_to(after.center())?;
    if centers.dot(normal)?.abs() > tolerance.absolute() {
        return Err(unsupported());
    }
    let distance = centers.length()?;
    if distance <= tolerance.absolute() {
        return Err(unsupported());
    }
    let radius_before = before.radius();
    let radius_after = after.radius();
    let along = (distance * distance + radius_before * radius_before - radius_after * radius_after)
        / (2.0 * distance);
    if !along.is_finite() || along.abs() > radius_before + tolerance.absolute() {
        return Err(unsupported());
    }
    let height = if (radius_before - along.abs()).abs() <= tolerance.absolute() {
        0.0
    } else {
        ((radius_before - along) * (radius_before + along)).sqrt()
    };
    let across = centers.normalized_nonzero()?.as_vector();
    let sideways = normal.cross(across)?.normalized_nonzero()?.as_vector();
    let base = before.center().translated(across.scaled(along)?)?;
    let mut best: Option<(Real, PolyCurve3, PolyCurve3)> = None;
    for sign in [-1.0, 1.0] {
        let meeting = base.translated(sideways.scaled(sign * height)?)?;
        let before_angle = circle_angle_at(before, meeting)?;
        let after_angle = circle_angle_at(after, meeting)?;
        if before_angle <= before.sweep_radians() + tolerance.angular()
            || before_angle >= std::f64::consts::TAU - tolerance.angular()
            || after_angle <= after.sweep_radians() + tolerance.angular()
            || after_angle >= std::f64::consts::TAU - tolerance.angular()
        {
            continue;
        }
        let Ok(before_extended) = before.try_extended_to_circle_angle(before_angle, true) else {
            continue;
        };
        let Ok(after_extended) = after.try_extended_to_circle_angle(after_angle, false) else {
            continue;
        };
        let mut first_segments = first.segments()[..first.segments().len() - 1].to_vec();
        first_segments.push(CurveSegment3::Arc(before_extended));
        let mut second_segments = vec![CurveSegment3::Arc(after_extended)];
        second_segments.extend_from_slice(&second.segments()[1..]);
        let Ok(first_result) = PolyCurve3::try_new(first_segments) else {
            continue;
        };
        let Ok(second_result) = PolyCurve3::try_new(second_segments) else {
            continue;
        };
        if before_extended
            .end()?
            .distance_to(after_extended.start()?)?
            > tolerance.absolute()
        {
            continue;
        }
        let extra_length = radius_before * (before_angle - before.sweep_radians())
            + radius_after * (std::f64::consts::TAU - after_angle);
        if best
            .as_ref()
            .is_none_or(|(prior, _, _)| extra_length < *prior)
        {
            best = Some((extra_length, first_result, second_result));
        }
    }
    best.map(|(_, first, second)| (first, second))
        .ok_or_else(unsupported)
}

fn circle_angle_at(arc: CircularArc3, point: Point3) -> Result<Real, GeometryError> {
    let radial = arc.center().vector_to(point)?;
    Ok(radial
        .dot(arc.y_axis().as_vector())?
        .atan2(radial.dot(arc.x_axis().as_vector())?)
        .rem_euclid(std::f64::consts::TAU))
}

/// Finds a circle/line meeting on the unused portion of the arc's circle.
/// The line support must lie in the arc plane; the chosen arc spans less than
/// one full revolution and retains its original radius, center, and direction.
fn connect_arc_line(
    first: &PolyCurve3,
    second: &PolyCurve3,
    arc: CircularArc3,
    line: LineSegment,
    arc_is_first: bool,
    tolerance: Tolerance,
) -> Result<(PolyCurve3, PolyCurve3), GeometryError> {
    let direction = line.direction(tolerance)?.as_vector();
    let normal = arc.normal()?.as_vector();
    let center_to_start = arc.center().vector_to(line.start())?;
    if center_to_start.dot(normal)?.abs() > tolerance.absolute()
        || direction.dot(normal)?.abs() > tolerance.angular().sin()
    {
        return Err(unsupported());
    }
    let along = center_to_start.dot(direction)?;
    let offset = center_to_start.to_array();
    let unit = direction.to_array();
    let perpendicular = Vector3::try_from(std::array::from_fn(|index| {
        (-along).mul_add(unit[index], offset[index])
    }))?;
    let radial_distance = perpendicular.length()?;
    if radial_distance > arc.radius() + tolerance.absolute() {
        return Err(unsupported());
    }
    let height = if (arc.radius() - radial_distance).abs() <= tolerance.absolute() {
        0.0
    } else {
        ((arc.radius() - radial_distance) * (arc.radius() + radial_distance)).sqrt()
    };
    let mut best: Option<(Real, PolyCurve3, PolyCurve3)> = None;
    for distance in [-along - height, -along + height] {
        let meeting = line.start().translated(direction.scaled(distance)?)?;
        let angle = circle_angle_at(arc, meeting)?;
        if angle <= arc.sweep_radians() + tolerance.angular()
            || angle >= std::f64::consts::TAU - tolerance.angular()
        {
            continue;
        }
        let (extended, line_segments, extra_length) = if arc_is_first {
            let Ok(extended) = arc.try_extended_to_circle_angle(angle, true) else {
                continue;
            };
            let Ok(line_segments) = retained_head(&CurveSegment3::Line(line), meeting, tolerance)
            else {
                continue;
            };
            (
                extended,
                line_segments,
                arc.radius() * (angle - arc.sweep_radians()),
            )
        } else {
            let Ok(extended) = arc.try_extended_to_circle_angle(angle, false) else {
                continue;
            };
            let Ok(line_segments) = retained_tail(&CurveSegment3::Line(line), meeting, tolerance)
            else {
                continue;
            };
            (
                extended,
                line_segments,
                arc.radius() * (std::f64::consts::TAU - angle),
            )
        };
        let (first_segments, second_segments) = if arc_is_first {
            let mut first_segments = first.segments()[..first.segments().len() - 1].to_vec();
            first_segments.push(CurveSegment3::Arc(extended));
            let mut second_segments = line_segments;
            second_segments.extend_from_slice(&second.segments()[1..]);
            (first_segments, second_segments)
        } else {
            let mut first_segments = first.segments()[..first.segments().len() - 1].to_vec();
            first_segments.extend(line_segments);
            let mut second_segments = vec![CurveSegment3::Arc(extended)];
            second_segments.extend_from_slice(&second.segments()[1..]);
            (first_segments, second_segments)
        };
        let Ok(first_result) = PolyCurve3::try_new(first_segments) else {
            continue;
        };
        let Ok(second_result) = PolyCurve3::try_new(second_segments) else {
            continue;
        };
        let first_end = first_result.evaluate(*first_result.domain().end())?;
        let second_start = second_result.evaluate(*second_result.domain().start())?;
        if first_end.distance_to(second_start)? > tolerance.absolute() {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(prior, _, _)| extra_length < *prior)
        {
            best = Some((extra_length, first_result, second_result));
        }
    }
    best.map(|(_, first, second)| (first, second))
        .ok_or_else(unsupported)
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
    use crate::NurbsCurve;

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
        let circular =
            try_connect_curves_joined(&arc, p(0., 1.), &second, p(-1., 2.), Tolerance::DEFAULT)
                .unwrap();
        assert!(matches!(
            circular.segments(),
            [CurveSegment3::Arc(_), CurveSegment3::Line(_)]
        ));
        let CurveSegment3::Arc(extended) = circular.segments()[0] else {
            unreachable!()
        };
        assert!(extended.end().unwrap().distance_to(p(-1., 0.)).unwrap() < 1e-12);
        assert!((extended.radius() - 1.).abs() < 1e-12);
        assert!((extended.sweep_radians() - std::f64::consts::PI).abs() < 1e-12);
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

    #[test]
    fn arc_extension_at_start_keeps_native_arc_and_source_direction() {
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
        let first = line(p(-1., -2.), p(-1., -1.));
        let parts =
            try_connect_curves_parts(&first, p(-1., -1.), &arc, p(1., 0.), Tolerance::DEFAULT)
                .unwrap();
        let Curve3::Arc(extended) = &parts[1] else {
            panic!("expected a native arc")
        };
        assert!(extended.start().unwrap().distance_to(p(-1., 0.)).unwrap() < 1e-12);
        assert!(extended.end().unwrap().distance_to(p(0., 1.)).unwrap() < 1e-12);
        assert!((extended.sweep_radians() - 1.5 * std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn arc_extension_selects_the_nearer_valid_circle_line_intersection() {
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
        let second = line(p(-0.5, 2.), p(-0.5, 3.));
        let joined =
            try_connect_curves_joined(&arc, p(0., 1.), &second, p(-0.5, 2.), Tolerance::DEFAULT)
                .unwrap();
        let CurveSegment3::Arc(extended) = joined.segments()[0] else {
            unreachable!()
        };
        assert!(
            extended
                .end()
                .unwrap()
                .distance_to(p(-0.5, 3.0_f64.sqrt() / 2.0))
                .unwrap()
                < 1e-12
        );
        assert!((extended.sweep_radians() - 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-12);
    }

    #[test]
    fn two_arcs_extend_on_their_supporting_circles() {
        let diagonal = 2.0_f64.sqrt() / 2.0;
        let first = Curve3::Arc(
            CircularArc3::try_from_three_points(
                p(1., 0.),
                p(diagonal, diagonal),
                p(0., 1.),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        let second = Curve3::Arc(
            CircularArc3::try_from_three_points(
                p(-3., 0.),
                p(-2. - diagonal, -diagonal),
                p(-2., -1.),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        let joined =
            try_connect_curves_joined(&first, p(0., 1.), &second, p(-3., 0.), Tolerance::DEFAULT)
                .unwrap();
        let [CurveSegment3::Arc(before), CurveSegment3::Arc(after)] = joined.segments() else {
            panic!("expected two native arcs")
        };
        assert!(before.end().unwrap().distance_to(p(-1., 0.)).unwrap() < 1e-12);
        assert!(after.start().unwrap().distance_to(p(-1., 0.)).unwrap() < 1e-12);
        assert!((before.sweep_radians() - std::f64::consts::PI).abs() < 1e-12);
        assert!((after.sweep_radians() - 1.5 * std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn two_arcs_choose_shorter_valid_intersection() {
        let diagonal = 2.0_f64.sqrt() / 2.0;
        let first = Curve3::Arc(
            CircularArc3::try_from_three_points(
                p(1., 0.),
                p(diagonal, diagonal),
                p(0., 1.),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        let second = Curve3::Arc(
            CircularArc3::try_from_three_points(
                p(-2., 0.),
                p(-1. - diagonal, -diagonal),
                p(-1., -1.),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        let joined =
            try_connect_curves_joined(&first, p(0., 1.), &second, p(-2., 0.), Tolerance::DEFAULT)
                .unwrap();
        let [CurveSegment3::Arc(before), CurveSegment3::Arc(after)] = joined.segments() else {
            panic!("expected two native arcs")
        };
        let meeting = p(-0.5, 3.0_f64.sqrt() / 2.0);
        assert!(before.end().unwrap().distance_to(meeting).unwrap() < 1e-12);
        assert!(after.start().unwrap().distance_to(meeting).unwrap() < 1e-12);
    }
}
