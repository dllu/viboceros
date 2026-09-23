//! Cyclic corner gaps and trims for closed, higher-degree planar curves.

use super::*;

pub(super) fn offset_nurbs_closed_gaps(
    curve: &NurbsCurve,
    distance: Real,
    fallback: UnitVector3,
    normal: UnitVector3,
    tolerance: Tolerance,
    cuts: &[(Real, bool)],
    seam: Option<bool>,
) -> Result<Vec<Curve3>, GeometryError> {
    let (sources, junctions) = if let Some(seam_convex) = seam {
        let domain = curve.domain();
        let mut parameters = Vec::with_capacity(cuts.len() + 2);
        parameters.push(*domain.start());
        parameters.extend(cuts.iter().map(|cut| cut.0));
        parameters.push(*domain.end());
        let sources = parameters
            .windows(2)
            .map(|window| curve.try_trimmed(window[0]..=window[1]))
            .collect::<Result<Vec<_>, _>>()?;
        let mut junctions = cuts.iter().map(|cut| cut.1).collect::<Vec<_>>();
        junctions.push(seam_convex);
        (sources, junctions)
    } else {
        let parameters = cuts.iter().map(|cut| cut.0).collect::<Vec<_>>();
        let sources = curve.try_split_at_parameters(&parameters)?;
        let junctions = (0..sources.len())
            .map(|index| cuts[(index + 1) % cuts.len()].1)
            .collect();
        (sources, junctions)
    };
    let offsets = sources
        .iter()
        .map(|source| offset_nurbs_with_closure(source, distance, fallback, tolerance, false))
        .collect::<Result<Vec<_>, _>>()?;
    let mut trims = offsets
        .iter()
        .map(|offset| [*offset.domain().start(), *offset.domain().end()])
        .collect::<Vec<_>>();
    for (index, &convex) in junctions.iter().enumerate() {
        if convex {
            continue;
        }
        if sources.len() == 1 {
            return Err(GeometryError::Degenerate {
                context: "single concave closed NURBS corner requires self-trimming",
            });
        }
        let next = (index + 1) % sources.len();
        let corner = sources[index].evaluate(*sources[index].domain().end())?;
        let mut best = None;
        for event in offsets[index].intersection_events_with_curve(&offsets[next], tolerance)? {
            let CurveCurveIntersectionEvent::Point(hit) = event else {
                return Err(GeometryError::Degenerate {
                    context: "overlapping closed NURBS offset corner",
                });
            };
            let (first, second) = (hit.first_parameter(), hit.second_parameter());
            if first <= trims[index][0]
                || first >= trims[index][1]
                || second <= trims[next][0]
                || second >= trims[next][1]
            {
                continue;
            }
            let first_tangent = offsets[index].derivative_at(first)?.normalized_nonzero()?;
            let second_tangent = offsets[next].derivative_at(second)?.normalized_nonzero()?;
            let crossing = first_tangent
                .as_vector()
                .cross(second_tangent.as_vector())?
                .dot(normal.as_vector())?
                .abs();
            if crossing <= tolerance.angular() {
                continue;
            }
            let proximity = hit.point().distance_to(corner)?;
            if best.is_none_or(|(previous, _, _)| proximity < previous) {
                best = Some((proximity, first, second));
            }
        }
        let Some((_, first, second)) = best else {
            return Err(GeometryError::Degenerate {
                context: "concave closed NURBS offset corner has no transverse trim",
            });
        };
        trims[index][1] = first;
        trims[next][0] = second;
    }
    let convex_gap = junctions.iter().rposition(|&convex| convex);
    if convex_gap.is_none() {
        return Ok(vec![build_group(
            &(0..sources.len()).collect::<Vec<_>>(),
            true,
            &sources,
            &offsets,
            &trims,
            distance,
            normal,
            tolerance,
        )?]);
    }
    let mut results = Vec::new();
    let mut group = Vec::new();
    let start = (convex_gap.expect("a convex gap exists") + 1) % sources.len();
    for step in 0..sources.len() {
        let index = (start + step) % sources.len();
        group.push(index);
        if junctions[index] {
            results.push(build_group(
                &group, false, &sources, &offsets, &trims, distance, normal, tolerance,
            )?);
            group.clear();
        }
    }
    debug_assert!(group.is_empty());
    Ok(results)
}

#[allow(clippy::too_many_arguments)]
fn build_group(
    indices: &[usize],
    closed: bool,
    sources: &[NurbsCurve],
    offsets: &[NurbsCurve],
    trims: &[[Real; 2]],
    distance: Real,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    let mut segments = Vec::with_capacity(indices.len());
    let mut breaks = vec![*sources[indices[0]].domain().start()];
    for &index in indices {
        if trims[index][0] >= trims[index][1] {
            return Err(GeometryError::Degenerate {
                context: "concave closed NURBS offset consumes a source segment",
            });
        }
        segments.push(offsets[index].try_trimmed(trims[index][0]..=trims[index][1])?);
        let source_domain = sources[index].domain();
        let previous = *breaks.last().unwrap();
        let end = if previous == *source_domain.start() {
            *source_domain.end()
        } else {
            previous + (*source_domain.end() - *source_domain.start())
        };
        crate::require_finite([end], "closed NURBS offset parameter")?;
        breaks.push(end);
    }
    for index in 0..segments.len().saturating_sub(1) {
        let (left, right) = segments.split_at_mut(index + 1);
        align_trimmed_ends(&mut left[index], &mut right[0], tolerance)?;
    }
    if closed {
        let (first, rest) = segments.split_first_mut().expect("a group has segments");
        let last = rest
            .last_mut()
            .expect("a closed group has multiple segments");
        align_trimmed_ends(last, first, tolerance)?;
    }
    for (&index, segment) in indices.iter().zip(&segments) {
        validate_trimmed_offset(&sources[index], segment, distance, normal, tolerance)?;
    }
    if segments.len() == 1 {
        return Ok(Curve3::NurbsCurve(segments.remove(0)));
    }
    Ok(Curve3::PolyCurve(PolyCurve3::try_with_segment_domains(
        segments
            .into_iter()
            .map(CurveSegment3::NurbsCurve)
            .collect::<Vec<_>>(),
        breaks,
    )?))
}
