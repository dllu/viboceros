use super::*;
use crate::{CircularArc3, CurveRef, LineSegment, curve_offset::nurbs_offset_plane};

const SAMPLES_PER_SPAN: usize = 48;
const MAX_ROOT_SAMPLES: usize = 65_536;

struct NurbsLineSolution {
    curve: NurbsCurve,
    fillet: CircularArc3,
    line: LineSegment,
    removed_length: Real,
}

pub(super) fn resolve_nurbs_line_kinks(
    source: &PolyCurve3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<PolyCurve3>, GeometryError> {
    let closed = source.is_closed()?;
    let mut segments = source.segments().to_vec();
    let mut changed = false;
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

fn fillet_pair(
    before: &CurveSegment3,
    after: &CurveSegment3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<(CurveSegment3, CircularArc3, CurveSegment3)>, GeometryError> {
    let (curve, line, reversed) = match (before, after) {
        (CurveSegment3::NurbsCurve(curve), CurveSegment3::Line(line)) => {
            (curve.clone(), *line, false)
        }
        (CurveSegment3::Line(line), CurveSegment3::NurbsCurve(curve)) => {
            (curve.reversed()?, line.reversed(), true)
        }
        _ => return Ok(None),
    };
    if straight_leaf_vertices(if reversed { after } else { before })?.is_some()
        || !arc_line::sharp_joint(before, after, tolerance)?
    {
        return Ok(None);
    }
    let solution = solve_nurbs_then_line(&curve, line, radius, tolerance)?;
    if reversed {
        Ok(Some((
            CurveSegment3::Line(solution.line.reversed()),
            solution.fillet.reversed(tolerance)?,
            CurveSegment3::NurbsCurve(solution.curve.reversed()?),
        )))
    } else {
        Ok(Some((
            CurveSegment3::NurbsCurve(solution.curve),
            solution.fillet,
            CurveSegment3::Line(solution.line),
        )))
    }
}

fn solve_nurbs_then_line(
    curve: &NurbsCurve,
    line: LineSegment,
    radius: Real,
    tolerance: Tolerance,
) -> Result<NurbsLineSolution, GeometryError> {
    let end_tangent = CurveRef::NurbsCurve(curve)
        .evaluate_with_tangent_on_side(*curve.domain().end(), ParameterSide::Left)?
        .tangent()
        .as_vector();
    let line_direction = line.direction(tolerance)?.as_vector();
    let fallback = end_tangent.cross(line_direction)?.normalized_nonzero()?;
    let normal = nurbs_offset_plane(curve, fallback, tolerance)?.as_vector();
    if normal.dot(line_direction)?.abs() > tolerance.angular() {
        return Err(unsupported_curved_corner());
    }
    let line_left = normal
        .cross(line_direction)?
        .normalized_nonzero()?
        .as_vector();
    let line_length = line.length()?;
    let spans = curve.spans().collect::<Vec<_>>();
    if spans.len().saturating_mul(SAMPLES_PER_SPAN) > MAX_ROOT_SAMPLES {
        return Err(GeometryError::InvalidPolyCurve {
            context: "too many NURBS fillet search intervals",
        });
    }
    let mut best: Option<NurbsLineSolution> = None;
    for curve_side in [-1.0, 1.0] {
        for line_side in [-1.0, 1.0] {
            let target = line_side * radius;
            for &(span_start, span_end) in &spans {
                let mut previous_parameter = span_start;
                let mut previous_value = offset_line_distance(
                    curve,
                    previous_parameter,
                    curve_side,
                    radius,
                    normal,
                    line.start(),
                    line_left,
                )? - target;
                for index in 1..=SAMPLES_PER_SPAN {
                    let fraction = index as Real / SAMPLES_PER_SPAN as Real;
                    let parameter = (span_end - span_start).mul_add(fraction, span_start);
                    let value = offset_line_distance(
                        curve,
                        parameter,
                        curve_side,
                        radius,
                        normal,
                        line.start(),
                        line_left,
                    )? - target;
                    let root = if previous_value == 0.0 {
                        Some(previous_parameter)
                    } else if value == 0.0 {
                        Some(parameter)
                    } else if previous_value.is_sign_negative() != value.is_sign_negative() {
                        Some(bisect_offset_root(
                            curve,
                            previous_parameter,
                            parameter,
                            previous_value,
                            curve_side,
                            radius,
                            normal,
                            line.start(),
                            line_left,
                            target,
                        )?)
                    } else {
                        None
                    };
                    if let Some(root) = root
                        && let Some(solution) = candidate_at(
                            curve,
                            line,
                            root,
                            curve_side,
                            radius,
                            normal,
                            line_direction,
                            line_left,
                            line_length,
                            target,
                            tolerance,
                        )?
                        && best.as_ref().is_none_or(|previous| {
                            solution.removed_length < previous.removed_length
                        })
                    {
                        best = Some(solution);
                    }
                    previous_parameter = parameter;
                    previous_value = value;
                }
            }
        }
    }
    best.ok_or_else(unsupported_curved_corner)
}

fn offset_line_distance(
    curve: &NurbsCurve,
    parameter: Real,
    side: Real,
    radius: Real,
    normal: Vector3,
    line_origin: Point3,
    line_left: Vector3,
) -> Result<Real, GeometryError> {
    let (_, _, center) = offset_sample(curve, parameter, side, radius, normal)?;
    line_origin.vector_to(center)?.dot(line_left)
}

fn offset_sample(
    curve: &NurbsCurve,
    parameter: Real,
    side: Real,
    radius: Real,
    normal: Vector3,
) -> Result<(Point3, Vector3, Point3), GeometryError> {
    let sample = CurveRef::NurbsCurve(curve).evaluate_with_tangent_on_side(
        parameter,
        if parameter == *curve.domain().start() {
            ParameterSide::Right
        } else {
            ParameterSide::Left
        },
    )?;
    let tangent = sample.tangent().as_vector();
    let left = normal.cross(tangent)?.normalized_nonzero()?.as_vector();
    let center = sample.point().translated(left.scaled(side * radius)?)?;
    Ok((sample.point(), tangent, center))
}

#[allow(clippy::too_many_arguments)]
fn bisect_offset_root(
    curve: &NurbsCurve,
    mut low: Real,
    mut high: Real,
    mut low_value: Real,
    side: Real,
    radius: Real,
    normal: Vector3,
    line_origin: Point3,
    line_left: Vector3,
    target: Real,
) -> Result<Real, GeometryError> {
    let mut best = if low_value.abs()
        <= (offset_line_distance(curve, high, side, radius, normal, line_origin, line_left)?
            - target)
            .abs()
    {
        low
    } else {
        high
    };
    let mut best_error =
        (offset_line_distance(curve, best, side, radius, normal, line_origin, line_left)? - target)
            .abs();
    for _ in 0..64 {
        let middle = low + (high - low) * 0.5;
        if middle == low || middle == high {
            break;
        }
        let value =
            offset_line_distance(curve, middle, side, radius, normal, line_origin, line_left)?
                - target;
        if value.abs() < best_error {
            best = middle;
            best_error = value.abs();
        }
        if value == 0.0 {
            break;
        }
        if value.is_sign_negative() == low_value.is_sign_negative() {
            low = middle;
            low_value = value;
        } else {
            high = middle;
        }
    }
    Ok(best)
}

#[allow(clippy::too_many_arguments)]
fn candidate_at(
    curve: &NurbsCurve,
    line: LineSegment,
    parameter: Real,
    curve_side: Real,
    radius: Real,
    normal: Vector3,
    line_direction: Vector3,
    line_left: Vector3,
    line_length: Real,
    target: Real,
    tolerance: Tolerance,
) -> Result<Option<NurbsLineSolution>, GeometryError> {
    if !(parameter > *curve.domain().start() && parameter < *curve.domain().end()) {
        return Ok(None);
    }
    let (point, tangent, center) = offset_sample(curve, parameter, curve_side, radius, normal)?;
    let residual = line.start().vector_to(center)?.dot(line_left)? - target;
    let scale = point.distance_to(line.start())?.max(radius).max(1.0);
    if residual.abs() > (tolerance.absolute() * 0.1).max(64.0 * Real::EPSILON * scale)
        || point.distance_to(curve.evaluate(*curve.domain().end())?)? <= tolerance.absolute()
    {
        return Ok(None);
    }
    let setback = line.start().vector_to(center)?.dot(line_direction)?;
    if !(setback > tolerance.absolute() && setback < line_length - tolerance.absolute()) {
        return Ok(None);
    }
    let line_contact = line.point_at(setback / line_length)?;
    let Some(fillet) = arc_line::tangent_fillet_arc(
        center,
        point,
        line_contact,
        tangent,
        line_direction,
        radius,
        tolerance,
    )?
    else {
        return Ok(None);
    };
    let removed_length = curve
        .try_trimmed(parameter..=*curve.domain().end())?
        .length(tolerance)?
        + setback;
    let line_parameter = CurveRef::Line(&line).parameter_at(setback / line_length)?;
    let CurveSegment3::Line(trimmed_line) =
        CurveSegment3::Line(line).try_trimmed(line_parameter..=*line.domain().end())?
    else {
        unreachable!()
    };
    Ok(Some(NurbsLineSolution {
        curve: curve.try_trimmed(*curve.domain().start()..=parameter)?,
        fillet,
        line: trimmed_line,
        removed_length,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }

    #[test]
    fn quadratic_nurbs_and_line_keep_native_leaves_and_tangent_fillet() {
        let curve = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(2., 0.), p(2., 2.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let line = LineSegment::try_new(p(2., 2.), p(6., 2.), Tolerance::DEFAULT).unwrap();
        for segments in [
            vec![
                CurveSegment3::NurbsCurve(curve.clone()),
                CurveSegment3::Line(line),
            ],
            vec![
                CurveSegment3::Line(line.reversed()),
                CurveSegment3::NurbsCurve(curve.reversed().unwrap()),
            ],
        ] {
            let source = PolyCurve3::try_new(segments).unwrap();
            let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
            assert_eq!(result.segments().len(), 3);
            assert!(matches!(
                result.segments()[0],
                CurveSegment3::NurbsCurve(_) | CurveSegment3::Line(_)
            ));
            let CurveSegment3::Arc(fillet) = result.segments()[1] else {
                panic!("middle leaf is the fillet")
            };
            assert!((fillet.radius() - 0.5).abs() < 1e-8);
            for pair in result.segments().windows(2) {
                assert!(!arc_line::sharp_joint(&pair[0], &pair[1], Tolerance::DEFAULT).unwrap());
            }
        }
    }
}
