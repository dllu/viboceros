//! Natural endpoint extensions for sharp NURBS offset corners.

use super::*;

pub(in crate::curve_offset) fn offset_nurbs_sharp(
    curve: &NurbsCurve,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    let pieces = offset_nurbs_open_gaps(curve, distance, fallback, tolerance)?;
    let closed = curve.is_closed()?;
    if pieces.len() == 1 && (!closed || pieces[0].as_ref().is_closed()?) {
        return Ok(pieces.into_iter().next().expect("one offset piece"));
    }
    let normal = offset_plane(curve, fallback, tolerance)?;
    let mut groups = pieces
        .iter()
        .map(|piece| match piece {
            Curve3::NurbsCurve(part) => Ok(vec![part.clone()]),
            Curve3::PolyCurve(part) => part
                .segments()
                .iter()
                .map(|segment| match segment {
                    CurveSegment3::NurbsCurve(leaf) => Ok(leaf.clone()),
                    _ => Err(GeometryError::Degenerate {
                        context: "non-NURBS sharp offset piece",
                    }),
                })
                .collect(),
            _ => Err(GeometryError::Degenerate {
                context: "non-NURBS sharp offset piece",
            }),
        })
        .collect::<Result<Vec<Vec<NurbsCurve>>, GeometryError>>()?;
    let gap_count = groups.len() - usize::from(!closed);
    let mut start_extensions = vec![None; groups.len()];
    let mut end_extensions = vec![None; groups.len()];
    for index in 0..gap_count {
        let next = (index + 1) % groups.len();
        let (end, start) = intersect_sharp_extensions(
            groups[index].last().expect("offset group has a leaf"),
            &groups[next][0],
            normal,
            tolerance,
        )?;
        end_extensions[index] = Some(end);
        start_extensions[next] = Some(start);
    }
    for (index, group) in groups.iter_mut().enumerate() {
        let last = group.len() - 1;
        if last == 0 {
            group[0] = extend_leaf(&group[0], start_extensions[index], end_extensions[index])?;
        } else {
            if let Some(start) = start_extensions[index] {
                group[0] = extend_leaf(&group[0], Some(start), None)?;
            }
            if let Some(end) = end_extensions[index] {
                group[last] = extend_leaf(&group[last], None, Some(end))?;
            }
        }
    }
    for index in 0..gap_count {
        let next = (index + 1) % groups.len();
        if index == next {
            if groups[index].len() == 1 {
                align_self_ends(&mut groups[index][0], tolerance)?;
            } else {
                let (first, rest) = groups[index]
                    .split_first_mut()
                    .expect("offset group has a leaf");
                align_trimmed_ends(rest.last_mut().expect("multiple leaves"), first, tolerance)?;
            }
        } else if index < next {
            let (before, after) = groups.split_at_mut(next);
            align_trimmed_ends(
                before[index].last_mut().expect("offset group has a leaf"),
                &mut after[0][0],
                tolerance,
            )?;
        } else {
            let (before, after) = groups.split_at_mut(index);
            align_trimmed_ends(
                after[0].last_mut().expect("offset group has a leaf"),
                &mut before[next][0],
                tolerance,
            )?;
        }
    }
    let first_parameter = if closed {
        match &pieces[0] {
            Curve3::NurbsCurve(part) => *part.domain().start(),
            Curve3::PolyCurve(part) => *part.domain().start(),
            _ => unreachable!("offset groups are NURBS or polycurves"),
        }
    } else {
        *curve.domain().start()
    };
    let end_parameter = first_parameter + (*curve.domain().end() - *curve.domain().start());
    crate::require_finite([end_parameter], "sharp NURBS offset parameter")?;
    let segments = groups
        .into_iter()
        .flatten()
        .map(CurveSegment3::NurbsCurve)
        .collect();
    let joined =
        PolyCurve3::try_new(segments)?.try_reparameterized(first_parameter..=end_parameter)?;
    if closed && !joined.is_closed()? {
        return Err(GeometryError::NurbsOffsetFitLimit);
    }
    Ok(Curve3::PolyCurve(joined))
}

fn extend_leaf(
    curve: &NurbsCurve,
    start: Option<Real>,
    end: Option<Real>,
) -> Result<NurbsCurve, GeometryError> {
    curve.try_extended_to(
        start.unwrap_or(*curve.domain().start())..=end.unwrap_or(*curve.domain().end()),
    )
}

fn align_self_ends(curve: &mut NurbsCurve, tolerance: Tolerance) -> Result<(), GeometryError> {
    let a = curve.evaluate(*curve.domain().start())?;
    let b = curve.evaluate(*curve.domain().end())?;
    if a.distance_to(b)? > tolerance.absolute() * 0.1 {
        return Err(GeometryError::NurbsOffsetFitLimit);
    }
    let shared = a.midpoint(b)?;
    let mut controls = curve.control_points().to_vec();
    let last = controls.len() - 1;
    controls[0] = WeightedPoint3::try_new(shared, controls[0].weight())?;
    controls[last] = WeightedPoint3::try_new(shared, controls[last].weight())?;
    *curve = NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())?;
    Ok(())
}

fn intersect_sharp_extensions(
    left: &NurbsCurve,
    right: &NurbsCurve,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<(Real, Real), GeometryError> {
    let left_domain = left.domain();
    let right_domain = right.domain();
    let a = left.evaluate(*left_domain.end())?;
    let b = right.evaluate(*right_domain.start())?;
    let u = left.derivative_at(*left_domain.end())?;
    let v = right.derivative_at(*right_domain.start())?;
    let denominator = u.cross(v)?.dot(normal.as_vector())?;
    let angular = u.length()? * v.length()? * tolerance.angular();
    if denominator.abs() <= angular {
        return Err(GeometryError::Degenerate {
            context: "parallel sharp NURBS offset extensions",
        });
    }
    let gap = a.vector_to(b)?;
    let left_delta = gap.cross(v)?.dot(normal.as_vector())? / denominator;
    let right_delta = gap.cross(u)?.dot(normal.as_vector())? / denominator;
    if left_delta <= 0.0 || right_delta >= 0.0 {
        return Err(GeometryError::Degenerate {
            context: "sharp NURBS offset extensions point away from their corner",
        });
    }
    let tail_start = left.spans().last().expect("offset has a span").0;
    let head_end = right.spans().next().expect("offset has a span").1;
    let tail = left.try_trimmed(tail_start..=*left_domain.end())?;
    let head = right.try_trimmed(*right_domain.start()..=head_end)?;
    let mut left_reach = left_delta * 2.0;
    let mut right_reach = -right_delta * 2.0;
    let mut best = None;
    for _ in 0..5 {
        let left_extended = tail.try_extended_to(tail_start..=*left_domain.end() + left_reach)?;
        let right_extended =
            head.try_extended_to(*right_domain.start() - right_reach..=head_end)?;
        for event in left_extended.intersection_events_with_curve(&right_extended, tolerance)? {
            let CurveCurveIntersectionEvent::Point(hit) = event else {
                continue;
            };
            let first = hit.first_parameter();
            let second = hit.second_parameter();
            if first <= *left_domain.end() || second >= *right_domain.start() {
                continue;
            }
            let first_tangent = left_extended.derivative_at(first)?.normalized_nonzero()?;
            let second_tangent = right_extended.derivative_at(second)?.normalized_nonzero()?;
            let crossing = first_tangent
                .as_vector()
                .cross(second_tangent.as_vector())?
                .dot(normal.as_vector())?
                .abs();
            if crossing <= tolerance.angular() {
                continue;
            }
            let proximity = hit.point().distance_to(a)? + hit.point().distance_to(b)?;
            if best.is_none_or(|(previous, _, _)| proximity < previous) {
                best = Some((proximity, first, second));
            }
        }
        if let Some((_, first, second)) = best {
            return Ok((first, second));
        }
        left_reach *= 2.0;
        right_reach *= 2.0;
    }
    Err(GeometryError::Degenerate {
        context: "sharp NURBS offset extensions do not intersect",
    })
}
