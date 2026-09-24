//! Endpoint connections using exact lines, circular supports, and tangents.

use crate::{
    CircularArc3, Curve3, CurveCurveIntersectionEvent, CurveExtensionSide, CurveExtensionStyle,
    CurveSegment3, GeometryError, LineSegment, NurbsCurve, ParameterSide, Point3, PolyCurve3, Real,
    Tolerance, UnitVector3, Vector3,
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

/// How a nonmeeting NURBS endpoint is extended by Connect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveOtherExtensionStyle {
    Line,
    Smooth,
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
    try_connect_curves_parts_with_styles(
        first,
        first_pick,
        second,
        second_pick,
        arc_extension,
        CurveOtherExtensionStyle::Line,
        tolerance,
    )
}

/// Connects selected ends with independent arc and other-curve extensions.
pub fn try_connect_curves_parts_with_styles(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
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
        other_extension,
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

/// Connects selected ends and returns updated picks at their meeting point.
/// The curves retain their original directions.
pub(crate) fn connected_ends_with_styles(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
    tolerance: Tolerance,
) -> Result<([Curve3; 2], [Point3; 2]), GeometryError> {
    let first_at_end = selected_end(first, first_pick, tolerance)?;
    let second_at_end = selected_end(second, second_pick, tolerance)?;
    let mut connected = try_connect_curves_parts_with_styles(
        first,
        first_pick,
        second,
        second_pick,
        arc_extension,
        other_extension,
        tolerance,
    )?;
    let second = connected.pop().expect("Connect returns two curves");
    let first = connected.pop().expect("Connect returns two curves");
    let first_pick = if first_at_end {
        first.as_ref().end_point()?
    } else {
        first.as_ref().start_point()?
    };
    let second_pick = if second_at_end {
        second.as_ref().end_point()?
    } else {
        second.as_ref().start_point()?
    };
    Ok(([first, second], [first_pick, second_pick]))
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
    try_connect_curves_joined_with_styles(
        first,
        first_pick,
        second,
        second_pick,
        arc_extension,
        CurveOtherExtensionStyle::Line,
        tolerance,
    )
}

/// Connects and joins with independent arc and other-curve extensions.
pub fn try_connect_curves_joined_with_styles(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    let (first, second) = connected_oriented(
        first,
        first_pick,
        second,
        second_pick,
        arc_extension,
        other_extension,
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
    other_extension: CurveOtherExtensionStyle,
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
    if other_extension == CurveOtherExtensionStyle::Smooth {
        match (tail, head) {
            (CurveSegment3::NurbsCurve(curve), CurveSegment3::Line(line)) => {
                return connect_smooth_nurbs_line(&first, &second, curve, *line, true, tolerance);
            }
            (CurveSegment3::Line(line), CurveSegment3::NurbsCurve(curve)) => {
                return connect_smooth_nurbs_line(&first, &second, curve, *line, false, tolerance);
            }
            (CurveSegment3::NurbsCurve(before), CurveSegment3::NurbsCurve(after)) => {
                return connect_smooth_nurbs_pair(&first, &second, before, after, tolerance);
            }
            _ => {}
        }
        if matches!(tail, CurveSegment3::NurbsCurve(_))
            || matches!(head, CurveSegment3::NurbsCurve(_))
        {
            return Err(unsupported());
        }
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

fn connect_smooth_nurbs_pair(
    first: &PolyCurve3,
    second: &PolyCurve3,
    before: &NurbsCurve,
    after: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<(PolyCurve3, PolyCurve3), GeometryError> {
    let before_domain = before.domain();
    let after_domain = after.domain();
    let before_end = *before_domain.end();
    let after_start = *after_domain.start();
    let a = before.evaluate(before_end)?;
    let b = after.evaluate(after_start)?;
    let before_span = before_end - before.spans().last().expect("NURBS has a span").0;
    let after_span = after.spans().next().expect("NURBS has a span").1 - after_start;
    let mut before_reach = before_span;
    let mut after_reach = after_span;
    for _ in 0..30 {
        let before_outer = before_end + before_reach;
        let after_outer = after_start - after_reach;
        if !before_outer.is_finite() || !after_outer.is_finite() {
            break;
        }
        let extended_before = before.try_extended_to(*before_domain.start()..=before_outer)?;
        let extended_after = after.try_extended_to(after_outer..=*after_domain.end())?;
        let mut best = None;
        for event in extended_before.intersection_events_with_curve(&extended_after, tolerance)? {
            let CurveCurveIntersectionEvent::Point(hit) = event else {
                continue;
            };
            let first_parameter = hit.first_parameter();
            let second_parameter = hit.second_parameter();
            if first_parameter <= before_end || second_parameter >= after_start {
                continue;
            }
            let proximity = a.distance_to(hit.point())? + b.distance_to(hit.point())?;
            if best.is_none_or(|(prior, _, _)| proximity < prior) {
                best = Some((proximity, first_parameter, second_parameter));
            }
        }
        if let Some((_, first_parameter, second_parameter)) = best {
            let before = extended_before.try_trimmed(*before_domain.start()..=first_parameter)?;
            let after = extended_after.try_trimmed(second_parameter..=*after_domain.end())?;
            let mut first_segments = first.segments()[..first.segments().len() - 1].to_vec();
            first_segments.push(CurveSegment3::NurbsCurve(before));
            let mut second_segments = vec![CurveSegment3::NurbsCurve(after)];
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
            return Ok((first, second));
        }
        before_reach *= 2.0;
        after_reach *= 2.0;
    }
    Err(unsupported())
}

fn connect_smooth_nurbs_line(
    first: &PolyCurve3,
    second: &PolyCurve3,
    curve: &NurbsCurve,
    line: LineSegment,
    nurbs_first: bool,
    tolerance: Tolerance,
) -> Result<(PolyCurve3, PolyCurve3), GeometryError> {
    let endpoint = curve.evaluate(if nurbs_first {
        *curve.domain().end()
    } else {
        *curve.domain().start()
    })?;
    let line_direction = line.direction(tolerance)?.as_vector();
    let reach = (endpoint.distance_to(line.start())? + endpoint.distance_to(line.end())?) * 8.0;
    if !reach.is_finite() || reach <= tolerance.absolute() {
        return Err(unsupported());
    }
    let support_start = line.start().translated(line_direction.scaled(-reach)?)?;
    let support_end = line.end().translated(line_direction.scaled(reach)?)?;
    let support = NurbsCurve::try_new(
        1,
        vec![support_start, support_end],
        vec![0.0, 0.0, 1.0, 1.0],
    )?;
    let side = if nurbs_first {
        CurveExtensionSide::End
    } else {
        CurveExtensionSide::Start
    };
    let extended = curve.try_merged_to_curve_boundaries(
        side,
        CurveExtensionStyle::Smooth,
        &[support],
        tolerance,
    )?;
    let meeting = extended.evaluate(if nurbs_first {
        *extended.domain().end()
    } else {
        *extended.domain().start()
    })?;
    let (first_segments, second_segments) = if nurbs_first {
        let mut first_segments = first.segments()[..first.segments().len() - 1].to_vec();
        first_segments.push(CurveSegment3::NurbsCurve(extended));
        let mut second_segments = retained_head(&CurveSegment3::Line(line), meeting, tolerance)?;
        second_segments.extend_from_slice(&second.segments()[1..]);
        (first_segments, second_segments)
    } else {
        let mut first_segments = first.segments()[..first.segments().len() - 1].to_vec();
        first_segments.extend(retained_tail(
            &CurveSegment3::Line(line),
            meeting,
            tolerance,
        )?);
        let mut second_segments = vec![CurveSegment3::NurbsCurve(extended)];
        second_segments.extend_from_slice(&second.segments()[1..]);
        (first_segments, second_segments)
    };
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
        let Some((before_adjusted, before_change)) =
            arc_at_circle_angle(before, before_angle, true, tolerance)?
        else {
            continue;
        };
        let Some((after_adjusted, after_change)) =
            arc_at_circle_angle(after, after_angle, false, tolerance)?
        else {
            continue;
        };
        let mut first_segments = first.segments()[..first.segments().len() - 1].to_vec();
        first_segments.push(CurveSegment3::Arc(before_adjusted));
        let mut second_segments = vec![CurveSegment3::Arc(after_adjusted)];
        second_segments.extend_from_slice(&second.segments()[1..]);
        let Ok(first_result) = PolyCurve3::try_new(first_segments) else {
            continue;
        };
        let Ok(second_result) = PolyCurve3::try_new(second_segments) else {
            continue;
        };
        if before_adjusted
            .end()?
            .distance_to(after_adjusted.start()?)?
            > tolerance.absolute()
        {
            continue;
        }
        let extra_length = before_change + after_change;
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

/// Adjusts the chosen arc endpoint to a point on its circle, retaining its
/// center and radius. A point inside the current sweep trims the chosen end;
/// a point in the unused sweep extends it without making a full circle.
fn arc_at_circle_angle(
    arc: CircularArc3,
    angle: Real,
    at_end: bool,
    tolerance: Tolerance,
) -> Result<Option<(CircularArc3, Real)>, GeometryError> {
    let sweep = arc.sweep_radians();
    let angular = tolerance.angular();
    if at_end && (angle - sweep).abs() <= angular {
        return Ok(Some((arc, 0.0)));
    }
    if !at_end && angle <= angular {
        return Ok(Some((arc, 0.0)));
    }
    if angle <= angular || angle >= std::f64::consts::TAU - angular {
        return Ok(None);
    }
    if angle > sweep + angular {
        let extended = arc.try_extended_to_circle_angle(angle, at_end)?;
        let added = if at_end {
            angle - sweep
        } else {
            std::f64::consts::TAU - angle
        };
        return Ok(Some((extended, arc.radius() * added)));
    }
    if angle >= sweep - angular {
        return Ok(None);
    }
    let domain = arc.domain();
    let parameter =
        *domain.start() + (*domain.end() - *domain.start()) * angle / arc.sweep_radians();
    let trimmed = if at_end {
        arc.try_trimmed(*domain.start()..=parameter)?
    } else {
        arc.try_trimmed(parameter..=*domain.end())?
    };
    let removed = if at_end { sweep - angle } else { angle };
    Ok(Some((trimmed, arc.radius() * removed)))
}

/// Finds a circle/line meeting for the selected arc end. The line support must
/// lie in the arc plane; the adjusted arc keeps its radius, center, and sense.
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
        let (extended, line_segments, extra_length) = if arc_is_first {
            let Some((extended, extra_length)) = arc_at_circle_angle(arc, angle, true, tolerance)?
            else {
                continue;
            };
            let Ok(line_segments) = retained_head(&CurveSegment3::Line(line), meeting, tolerance)
            else {
                continue;
            };
            (extended, line_segments, extra_length)
        } else {
            let Some((extended, extra_length)) = arc_at_circle_angle(arc, angle, false, tolerance)?
            else {
                continue;
            };
            let Ok(line_segments) = retained_tail(&CurveSegment3::Line(line), meeting, tolerance)
            else {
                continue;
            };
            (extended, line_segments, extra_length)
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
    fn smooth_extension_matches_rhino_quadratic_line_meeting() {
        let first = curve(p(0., 0.), p(1., 0.), p(2., 1.));
        let second = line(p(3., 3.), p(3., 4.));
        let joined = try_connect_curves_joined_with_styles(
            &first,
            p(2., 1.),
            &second,
            p(3., 3.),
            CurveArcExtensionStyle::Arc,
            CurveOtherExtensionStyle::Smooth,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [
            CurveSegment3::NurbsCurve(extended),
            CurveSegment3::Line(retained),
        ] = joined.segments()
        else {
            panic!("smooth NURBS and retained line");
        };
        assert_eq!(extended.degree(), 2);
        assert!((extended.domain().end() - 1.5).abs() < 1e-10);
        for (control, expected) in
            extended
                .control_points()
                .iter()
                .zip([p(0., 0.), p(1.5, 0.), p(3., 2.25)])
        {
            assert!(control.point().distance_to(expected).unwrap() < 1e-10);
        }
        assert!(retained.start().distance_to(p(3., 2.25)).unwrap() < 1e-10);
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
    fn arc_end_trims_to_meet_a_line() {
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
        let line = line(p(0.5, 2.), p(0.5, 3.));
        let parts =
            try_connect_curves_parts(&arc, p(0., 1.), &line, p(0.5, 2.), Tolerance::DEFAULT)
                .unwrap();
        let Curve3::Arc(trimmed) = parts[0] else {
            panic!("expected a native trimmed arc")
        };
        assert!(
            trimmed
                .end()
                .unwrap()
                .distance_to(p(0.5, 3.0_f64.sqrt() / 2.0))
                .unwrap()
                < 1e-12
        );
        assert!((trimmed.sweep_radians() - std::f64::consts::PI / 3.0).abs() < 1e-12);
    }

    #[test]
    fn arc_start_trims_to_meet_a_line() {
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
        let x = 3.0_f64.sqrt() / 2.0;
        let line = line(p(x, 0.), p(x, 0.1));
        let parts = try_connect_curves_parts(&line, p(x, 0.1), &arc, p(1., 0.), Tolerance::DEFAULT)
            .unwrap();
        let Curve3::Arc(trimmed) = parts[1] else {
            panic!("expected a native trimmed arc")
        };
        assert!(trimmed.start().unwrap().distance_to(p(x, 0.5)).unwrap() < 1e-12);
        assert!(trimmed.end().unwrap().distance_to(p(0., 1.)).unwrap() < 1e-12);
        assert!((trimmed.sweep_radians() - std::f64::consts::PI / 3.0).abs() < 1e-12);
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

    #[test]
    fn two_arcs_trim_their_selected_ends_to_crossing() {
        let root_three = 3.0_f64.sqrt();
        let diagonal = 2.0_f64.sqrt() / 2.0;
        let first = Curve3::Arc(
            CircularArc3::try_from_three_points(
                p(1., 0.),
                p(0., 1.),
                p(-root_three / 2., 0.5),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        let second = Curve3::Arc(
            CircularArc3::try_from_three_points(
                p(0., 0.),
                p(-1. + diagonal, diagonal),
                p(-1., 1.),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        let parts = try_connect_curves_parts(
            &first,
            p(-root_three / 2., 0.5),
            &second,
            p(0., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [Curve3::Arc(before), Curve3::Arc(after)] = parts.as_slice() else {
            panic!("expected two native trimmed arcs")
        };
        let meeting = p(-0.5, root_three / 2.);
        assert!(before.end().unwrap().distance_to(meeting).unwrap() < 1e-12);
        assert!(after.start().unwrap().distance_to(meeting).unwrap() < 1e-12);
        assert!((before.sweep_radians() - 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-12);
        assert!((after.sweep_radians() - std::f64::consts::PI / 6.0).abs() < 1e-12);
    }
}
