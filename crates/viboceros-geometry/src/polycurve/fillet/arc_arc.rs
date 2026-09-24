use super::*;
use crate::{CircularArc3, CurveRef};

struct ArcArcSolution {
    before: CircularArc3,
    fillet: CircularArc3,
    after: CircularArc3,
    removed_length: Real,
}

pub(super) fn resolve_arc_arc_kinks(
    source: &PolyCurve3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<PolyCurve3>, GeometryError> {
    let closed = source.is_closed()?;
    let mut segments = source.segments().to_vec();
    let mut changed = false;
    let mut index = 0;
    while index + 1 < segments.len() {
        if let Some(solution) =
            fillet_pair(&segments[index], &segments[index + 1], radius, tolerance)?
        {
            segments.splice(
                index..=index + 1,
                [
                    CurveSegment3::Arc(solution.before),
                    CurveSegment3::Arc(solution.fillet),
                    CurveSegment3::Arc(solution.after),
                ],
            );
            changed = true;
            index += 2;
        } else {
            index += 1;
        }
    }
    if closed
        && let Some(solution) =
            fillet_pair(segments.last().unwrap(), &segments[0], radius, tolerance)?
    {
        segments[0] = CurveSegment3::Arc(solution.after);
        *segments.last_mut().unwrap() = CurveSegment3::Arc(solution.before);
        segments.insert(0, CurveSegment3::Arc(solution.fillet));
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
) -> Result<Option<ArcArcSolution>, GeometryError> {
    let (CurveSegment3::Arc(first), CurveSegment3::Arc(second)) = (before, after) else {
        return Ok(None);
    };
    if !arc_line::sharp_joint(before, after, tolerance)? {
        return Ok(None);
    }
    Ok(Some(solve_arc_arc(*first, *second, radius, tolerance)?))
}

fn solve_arc_arc(
    before: CircularArc3,
    after: CircularArc3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<ArcArcSolution, GeometryError> {
    let first_center = before.center();
    let second_center = after.center();
    let first_normal = before.normal()?.as_vector();
    let second_normal = after.normal()?.as_vector();
    if first_normal.cross(second_normal)?.length()? > tolerance.angular() {
        return Err(unsupported_curved_corner());
    }
    let between = first_center.vector_to(second_center)?;
    if between.dot(first_normal)?.abs() > tolerance.absolute() {
        return Err(unsupported_curved_corner());
    }
    let distance = between.length()?;
    if distance <= tolerance.absolute() {
        return Err(unsupported_curved_corner());
    }
    let axis = between.normalized_nonzero()?.as_vector();
    let perpendicular = first_normal.cross(axis)?.normalized_nonzero()?.as_vector();
    let mut best: Option<ArcArcSolution> = None;
    for first_signed_radius in [before.radius() + radius, before.radius() - radius] {
        if first_signed_radius == 0.0 {
            continue;
        }
        for second_signed_radius in [after.radius() + radius, after.radius() - radius] {
            if second_signed_radius == 0.0 {
                continue;
            }
            let first_distance = first_signed_radius.abs();
            let second_distance = second_signed_radius.abs();
            let along = (first_distance * first_distance - second_distance * second_distance
                + distance * distance)
                / (2.0 * distance);
            let height_squared = first_distance * first_distance - along * along;
            if !height_squared.is_finite() || height_squared < 0.0 {
                continue;
            }
            let foot = first_center.translated(axis.scaled(along)?)?;
            let height = height_squared.sqrt();
            for side in [-1.0, 1.0] {
                let fillet_center = foot.translated(perpendicular.scaled(side * height)?)?;
                let first_radial = first_center
                    .vector_to(fillet_center)?
                    .scaled(1.0 / first_signed_radius)?;
                let second_radial = second_center
                    .vector_to(fillet_center)?
                    .scaled(1.0 / second_signed_radius)?;
                let first_contact =
                    first_center.translated(first_radial.scaled(before.radius())?)?;
                let second_contact =
                    second_center.translated(second_radial.scaled(after.radius())?)?;
                let first_parameter =
                    CurveRef::Arc(&before).closest_parameter(first_contact, tolerance)?;
                let second_parameter =
                    CurveRef::Arc(&after).closest_parameter(second_contact, tolerance)?;
                if !(first_parameter > *before.domain().start()
                    && first_parameter < *before.domain().end()
                    && second_parameter > *after.domain().start()
                    && second_parameter < *after.domain().end())
                {
                    continue;
                }
                let first_contact_exact = before.evaluate(first_parameter)?;
                let second_contact_exact = after.evaluate(second_parameter)?;
                if first_contact.distance_to(first_contact_exact)? > tolerance.absolute()
                    || second_contact.distance_to(second_contact_exact)? > tolerance.absolute()
                    || first_contact_exact.distance_to(before.end()?)? <= tolerance.absolute()
                    || second_contact_exact.distance_to(after.start()?)? <= tolerance.absolute()
                {
                    continue;
                }
                let first_tangent = CurveRef::Arc(&before)
                    .evaluate_with_tangent_on_side(first_parameter, ParameterSide::Right)?
                    .tangent()
                    .as_vector();
                let second_tangent = CurveRef::Arc(&after)
                    .evaluate_with_tangent_on_side(second_parameter, ParameterSide::Left)?
                    .tangent()
                    .as_vector();
                let Some(fillet) = arc_line::tangent_fillet_arc(
                    fillet_center,
                    first_contact_exact,
                    second_contact_exact,
                    first_tangent,
                    second_tangent,
                    radius,
                    tolerance,
                )?
                else {
                    continue;
                };
                let removed_length = before.radius()
                    * before.sweep_radians()
                    * ((*before.domain().end() - first_parameter)
                        / (*before.domain().end() - *before.domain().start()))
                    + after.radius()
                        * after.sweep_radians()
                        * ((second_parameter - *after.domain().start())
                            / (*after.domain().end() - *after.domain().start()));
                let solution = ArcArcSolution {
                    before: before.try_trimmed(*before.domain().start()..=first_parameter)?,
                    fillet,
                    after: after.try_trimmed(second_parameter..=*after.domain().end())?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineSegment;

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }

    #[test]
    fn tangent_arc_connects_two_trimmed_native_arcs() {
        let diagonal = 2_f64.sqrt();
        let first = CircularArc3::try_from_three_points(
            p(0., 0.),
            p(diagonal, 2. - diagonal),
            p(2., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let second = CircularArc3::try_from_three_points(
            p(2., 2.),
            p(2. + diagonal, 4. - diagonal),
            p(4., 4.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source =
            PolyCurve3::try_new(vec![CurveSegment3::Arc(first), CurveSegment3::Arc(second)])
                .unwrap();
        let rounded = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert_eq!(rounded.segments().len(), 3);
        let CurveSegment3::Arc(fillet) = rounded.segments()[1] else {
            panic!("middle segment is the fillet")
        };
        assert!((fillet.radius() - 0.5).abs() < 1e-10);
        for pair in rounded.segments().windows(2) {
            assert!(!arc_line::sharp_joint(&pair[0], &pair[1], Tolerance::DEFAULT).unwrap());
        }
        let reversed = PolyCurve3::try_new(vec![
            CurveSegment3::Arc(second.reversed(Tolerance::DEFAULT).unwrap()),
            CurveSegment3::Arc(first.reversed(Tolerance::DEFAULT).unwrap()),
        ])
        .unwrap()
        .try_fillet_corners(0.5, Tolerance::DEFAULT)
        .unwrap();
        assert_eq!(reversed.segments().len(), 3);
        assert!(
            (reversed.length(Tolerance::DEFAULT).unwrap()
                - rounded.length(Tolerance::DEFAULT).unwrap())
            .abs()
                < 1e-11
        );

        let closed = PolyCurve3::try_new(vec![
            CurveSegment3::Arc(first),
            CurveSegment3::Arc(second),
            CurveSegment3::Line(
                LineSegment::try_new(p(4., 4.), p(0., 4.), Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::Line(
                LineSegment::try_new(p(0., 4.), p(0., 0.), Tolerance::DEFAULT).unwrap(),
            ),
        ])
        .unwrap();
        let closed_rounded = closed.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert!(closed_rounded.is_closed().unwrap());
        assert!(matches!(
            closed_rounded.segments()[0],
            CurveSegment3::Arc(_)
        ));
        assert!(closed_rounded.segments().len() >= 8);
    }
}
