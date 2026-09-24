use super::*;
use crate::{CircularArc3, CurveRef, LineSegment};

struct ArcLineSolution {
    arc: CircularArc3,
    fillet: CircularArc3,
    line: LineSegment,
    removed_length: Real,
}

pub(super) fn resolve_arc_line_kinks(
    source: &PolyCurve3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<PolyCurve3>, GeometryError> {
    if !source
        .segments()
        .iter()
        .any(|segment| matches!(segment, CurveSegment3::Arc(_)))
    {
        return Ok(None);
    }
    let closed = source.is_closed()?;
    let (mut segments, mut changed) = expand_straight_arc_neighbors(source, closed, tolerance)?;
    let mut index = 0;
    while index + 1 < segments.len() {
        if let Some((before, fillet, after)) =
            fillet_pair(&segments[index], &segments[index + 1], radius, tolerance)?
        {
            segments.splice(
                index..=index + 1,
                [before, CurveSegment3::Arc(fillet), after],
            );
            changed = true;
            index += 2;
        } else {
            index += 1;
        }
    }
    if closed
        && let Some((before, fillet, after)) =
            fillet_pair(segments.last().unwrap(), &segments[0], radius, tolerance)?
    {
        segments[0] = after;
        *segments.last_mut().unwrap() = before;
        segments.insert(0, CurveSegment3::Arc(fillet));
        changed = true;
    }
    if changed {
        Ok(Some(PolyCurve3::try_new(segments)?))
    } else {
        Ok(None)
    }
}

fn expand_straight_arc_neighbors(
    source: &PolyCurve3,
    closed: bool,
    tolerance: Tolerance,
) -> Result<(Vec<CurveSegment3>, bool), GeometryError> {
    let leaves = source.segments();
    let mut expand = vec![false; leaves.len()];
    for index in 0..leaves.len() - 1 + usize::from(closed) {
        let next = (index + 1) % leaves.len();
        let straight_index = match (&leaves[index], &leaves[next]) {
            (CurveSegment3::Arc(_), CurveSegment3::Polyline(_) | CurveSegment3::NurbsCurve(_)) => {
                next
            }
            (CurveSegment3::Polyline(_) | CurveSegment3::NurbsCurve(_), CurveSegment3::Arc(_)) => {
                index
            }
            _ => continue,
        };
        if straight_leaf_vertices(&leaves[straight_index])?.is_some()
            && sharp_joint(&leaves[index], &leaves[next], tolerance)?
        {
            expand[straight_index] = true;
        }
    }
    let changed = expand.iter().any(|&flag| flag);
    if !changed {
        return Ok((leaves.to_vec(), false));
    }
    let mut segments = Vec::new();
    for (segment, expand) in leaves.iter().zip(expand) {
        if expand {
            let points = straight_leaf_vertices(segment)?.expect("marked leaf is straight");
            for pair in points.windows(2) {
                if segments.len() >= MAX_POLYCURVE_SEGMENTS {
                    return Err(GeometryError::InvalidPolyCurve {
                        context: "too many fillet segments",
                    });
                }
                segments.push(CurveSegment3::Line(LineSegment::try_new(
                    pair[0],
                    pair[1],
                    Tolerance::NUMERICAL_VALIDATION,
                )?));
            }
        } else {
            segments.push(segment.clone());
        }
    }
    Ok((segments, true))
}

fn sharp_joint(
    before: &CurveSegment3,
    after: &CurveSegment3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let before_sample = before
        .as_ref()
        .evaluate_with_tangent_on_side(*before.domain().end(), ParameterSide::Left)?;
    let after_sample = after
        .as_ref()
        .evaluate_with_tangent_on_side(*after.domain().start(), ParameterSide::Right)?;
    Ok(tangent_angle(
        before_sample.tangent().as_vector(),
        after_sample.tangent().as_vector(),
    )? > tolerance.angular())
}

fn fillet_pair(
    before: &CurveSegment3,
    after: &CurveSegment3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<(CurveSegment3, CircularArc3, CurveSegment3)>, GeometryError> {
    let (arc, line, reversed) = match (before, after) {
        (CurveSegment3::Arc(arc), CurveSegment3::Line(line)) => (*arc, *line, false),
        (CurveSegment3::Line(line), CurveSegment3::Arc(arc)) => {
            (arc.reversed(tolerance)?, line.reversed(), true)
        }
        _ => return Ok(None),
    };
    if !sharp_joint(before, after, tolerance)? {
        return Ok(None);
    }
    let solution = solve_arc_then_line(arc, line, radius, tolerance)?;
    if reversed {
        Ok(Some((
            CurveSegment3::Line(solution.line.reversed()),
            solution.fillet.reversed(tolerance)?,
            CurveSegment3::Arc(solution.arc.reversed(tolerance)?),
        )))
    } else {
        Ok(Some((
            CurveSegment3::Arc(solution.arc),
            solution.fillet,
            CurveSegment3::Line(solution.line),
        )))
    }
}

fn solve_arc_then_line(
    arc: CircularArc3,
    line: LineSegment,
    radius: Real,
    tolerance: Tolerance,
) -> Result<ArcLineSolution, GeometryError> {
    let center = arc.center();
    let arc_radius = arc.radius();
    let normal = arc.normal()?.as_vector();
    let direction = line.direction(tolerance)?.as_vector();
    if normal.dot(direction)?.abs() > tolerance.angular() {
        return Err(unsupported_curved_corner());
    }
    let left = normal.cross(direction)?.normalized_nonzero()?.as_vector();
    let length = line.length()?;
    let mut best: Option<ArcLineSolution> = None;
    for side in [-1.0, 1.0] {
        let offset_origin = line.start().translated(left.scaled(side * radius)?)?;
        let center_to_origin = center.vector_to(offset_origin)?;
        let along = center_to_origin.dot(direction)?;
        let perpendicular = subtract(center_to_origin, direction.scaled(along)?)?;
        let perpendicular_squared = perpendicular.dot(perpendicular)?;
        for signed_radius in [arc_radius + radius, arc_radius - radius] {
            if signed_radius == 0.0 {
                continue;
            }
            let height_squared = signed_radius * signed_radius - perpendicular_squared;
            if !height_squared.is_finite() || height_squared < 0.0 {
                continue;
            }
            let height = height_squared.sqrt();
            for line_setback in [-along - height, -along + height] {
                if !(line_setback > tolerance.absolute()
                    && line_setback < length - tolerance.absolute())
                {
                    continue;
                }
                let fillet_center = offset_origin.translated(direction.scaled(line_setback)?)?;
                let radial = center
                    .vector_to(fillet_center)?
                    .scaled(1.0 / signed_radius)?;
                let arc_contact = center.translated(radial.scaled(arc_radius)?)?;
                let parameter = CurveRef::Arc(&arc).closest_parameter(arc_contact, tolerance)?;
                if !(parameter > *arc.domain().start() && parameter < *arc.domain().end()) {
                    continue;
                }
                let contact = arc.evaluate(parameter)?;
                if contact.distance_to(arc_contact)? > tolerance.absolute()
                    || contact.distance_to(arc.end()?)? <= tolerance.absolute()
                {
                    continue;
                }
                let line_contact = line.point_at(line_setback / length)?;
                let source_tangent = CurveRef::Arc(&arc)
                    .evaluate_with_tangent_on_side(parameter, ParameterSide::Right)?
                    .tangent()
                    .as_vector();
                let Some(fillet) = tangent_fillet_arc(
                    fillet_center,
                    contact,
                    line_contact,
                    source_tangent,
                    direction,
                    radius,
                    tolerance,
                )?
                else {
                    continue;
                };
                let trimmed_arc = arc.try_trimmed(*arc.domain().start()..=parameter)?;
                let line_parameter = CurveRef::Line(&line).parameter_at(line_setback / length)?;
                let trimmed_line =
                    CurveSegment3::Line(line).try_trimmed(line_parameter..=*line.domain().end())?;
                let CurveSegment3::Line(trimmed_line) = trimmed_line else {
                    unreachable!()
                };
                let removed_length = arc_radius
                    * arc.sweep_radians()
                    * ((*arc.domain().end() - parameter)
                        / (*arc.domain().end() - *arc.domain().start()))
                    + line_setback;
                let solution = ArcLineSolution {
                    arc: trimmed_arc,
                    fillet,
                    line: trimmed_line,
                    removed_length,
                };
                if best
                    .as_ref()
                    .is_none_or(|previous| removed_length < previous.removed_length)
                {
                    best = Some(solution);
                }
            }
        }
    }
    best.ok_or_else(unsupported_curved_corner)
}

fn tangent_fillet_arc(
    center: Point3,
    start: Point3,
    end: Point3,
    start_tangent: Vector3,
    end_tangent: Vector3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<CircularArc3>, GeometryError> {
    let first = center.vector_to(start)?.normalized_nonzero()?.as_vector();
    let last = center.vector_to(end)?.normalized_nonzero()?.as_vector();
    let normal = first
        .cross(start_tangent)?
        .normalized_nonzero()?
        .as_vector();
    if tangent_angle(normal.cross(last)?, end_tangent)? > tolerance.angular() {
        return Ok(None);
    }
    let turn = normal.dot(first.cross(last)?)?.atan2(first.dot(last)?);
    let turn = turn.rem_euclid(std::f64::consts::TAU);
    if !(turn > tolerance.angular() && turn < std::f64::consts::PI) {
        return Ok(None);
    }
    let (sine, cosine) = (turn * 0.5).sin_cos();
    let middle_direction = add(first.scaled(cosine)?, normal.cross(first)?.scaled(sine)?)?;
    let middle = center.translated(middle_direction.scaled(radius)?)?;
    let Ok(fillet) = CircularArc3::try_from_three_points(start, middle, end, tolerance) else {
        return Ok(None);
    };
    Ok(Some(fillet))
}

fn add(a: Vector3, b: Vector3) -> Result<Vector3, GeometryError> {
    Vector3::try_new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z())
}

fn subtract(a: Vector3, b: Vector3) -> Result<Vector3, GeometryError> {
    Vector3::try_new(a.x() - b.x(), a.y() - b.y(), a.z() - b.z())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }

    #[test]
    fn arc_line_kink_has_native_tangent_fillet_in_either_direction() {
        let arc = CircularArc3::try_from_three_points(
            p(0., 0.),
            p(2_f64.sqrt(), 2. - 2_f64.sqrt()),
            p(2., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let line = LineSegment::try_new(p(2., 2.), p(6., 2.), Tolerance::DEFAULT).unwrap();
        for segments in [
            vec![CurveSegment3::Arc(arc), CurveSegment3::Line(line)],
            vec![
                CurveSegment3::Line(line.reversed()),
                CurveSegment3::Arc(arc.reversed(Tolerance::DEFAULT).unwrap()),
            ],
        ] {
            let source = PolyCurve3::try_new(segments).unwrap();
            let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
            assert_eq!(result.segments().len(), 3);
            let CurveSegment3::Arc(fillet) = result.segments()[1] else {
                panic!("middle leaf is a fillet arc")
            };
            assert!((fillet.radius() - 0.5).abs() < 1e-10);
            for pair in result.segments().windows(2) {
                let outgoing = pair[0]
                    .as_ref()
                    .evaluate_with_tangent_on_side(*pair[0].domain().end(), ParameterSide::Left)
                    .unwrap();
                let incoming = pair[1]
                    .as_ref()
                    .evaluate_with_tangent_on_side(*pair[1].domain().start(), ParameterSide::Right)
                    .unwrap();
                assert!(
                    tangent_angle(
                        outgoing.tangent().as_vector(),
                        incoming.tangent().as_vector()
                    )
                    .unwrap()
                        < 1e-10
                );
            }
            assert!(
                result
                    .segments()
                    .iter()
                    .all(|part| matches!(part, CurveSegment3::Arc(_) | CurveSegment3::Line(_)))
            );
            let length = result.length(Tolerance::DEFAULT).unwrap();
            assert!((length - 6.974106275595999).abs() < 1e-8, "{length}");
            assert!(
                result
                    .evaluate(*result.domain().start())
                    .unwrap()
                    .distance_to(source.evaluate(*source.domain().start()).unwrap())
                    .unwrap()
                    < 1e-12
            );
            assert!(
                result
                    .evaluate(*result.domain().end())
                    .unwrap()
                    .distance_to(source.evaluate(*source.domain().end()).unwrap())
                    .unwrap()
                    < 1e-12
            );
            assert!(source.try_fillet_corners(10., Tolerance::DEFAULT).is_err());
        }
    }

    #[test]
    fn arc_line_joints_combine_with_straight_corners_and_closed_seam() {
        let arc = CircularArc3::try_from_three_points(
            p(0., 0.),
            p(2_f64.sqrt(), 2. - 2_f64.sqrt()),
            p(2., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let line =
            |a, b| CurveSegment3::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap());
        let open = PolyCurve3::try_new(vec![
            CurveSegment3::Arc(arc),
            line(p(2., 2.), p(6., 2.)),
            line(p(6., 2.), p(6., 6.)),
        ])
        .unwrap();
        let rounded = open.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert_eq!(rounded.segments().len(), 5);
        assert!(matches!(rounded.segments()[0], CurveSegment3::Arc(_)));
        assert!(matches!(rounded.segments()[1], CurveSegment3::Arc(_)));
        assert!(matches!(rounded.segments()[3], CurveSegment3::Arc(_)));

        let closed = PolyCurve3::try_new(vec![
            CurveSegment3::Arc(arc),
            line(p(2., 2.), p(6., 2.)),
            line(p(6., 2.), p(6., -2.)),
            line(p(6., -2.), p(0., -2.)),
            line(p(0., -2.), p(0., 0.)),
        ])
        .unwrap();
        let rounded = closed.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert!(rounded.is_closed().unwrap());
        assert!(matches!(rounded.segments()[0], CurveSegment3::Arc(_)));
        assert!(rounded.segments().len() >= 9);
    }

    #[test]
    fn polyline_and_linear_nurbs_neighbors_match_their_exact_line_spans() {
        let arc = CircularArc3::try_from_three_points(
            p(0., 0.),
            p(2_f64.sqrt(), 2. - 2_f64.sqrt()),
            p(2., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let vertices = vec![p(2., 2.), p(6., 2.), p(6., 6.)];
        let reference = PolyCurve3::try_new(vec![
            CurveSegment3::Arc(arc),
            CurveSegment3::Line(
                LineSegment::try_new(vertices[0], vertices[1], Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::Line(
                LineSegment::try_new(vertices[1], vertices[2], Tolerance::DEFAULT).unwrap(),
            ),
        ])
        .unwrap()
        .try_fillet_corners(0.5, Tolerance::DEFAULT)
        .unwrap();
        let leaves = [
            CurveSegment3::Polyline(
                Polyline3::try_new(vertices.clone(), Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::NurbsCurve(
                NurbsCurve::try_new(1, vertices, vec![0., 0., 1., 2., 2.]).unwrap(),
            ),
        ];
        for leaf in leaves {
            let source = PolyCurve3::try_new(vec![CurveSegment3::Arc(arc), leaf]).unwrap();
            let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
            assert_eq!(result.segments().len(), 5);
            let actual = CurveRef::PolyCurve(&result)
                .sample_equal_length_points(16, true, Tolerance::DEFAULT)
                .unwrap();
            let expected = CurveRef::PolyCurve(&reference)
                .sample_equal_length_points(16, true, Tolerance::DEFAULT)
                .unwrap();
            for (a, b) in actual.into_iter().zip(expected) {
                assert!(a.distance_to(b).unwrap() < 1e-12);
            }
        }
    }
}
