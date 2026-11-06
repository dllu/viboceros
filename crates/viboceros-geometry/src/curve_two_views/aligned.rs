//! Exact shared-chart reconstruction; turning profiles require the intersector.
use super::*;
use crate::WeightedPoint3;

pub(super) fn reconstruct(
    curves: [&NurbsCurve; 2],
    [a, b]: [Vector3; 2],
) -> Result<Option<NurbsCurve>, GeometryError> {
    let axis = a.cross(b)?;
    let origin = curves[0].control_points()[0].point();
    let h = |p| axis.dot_point_difference(p, origin);
    let endpoints = curves.map(|c| {
        c.evaluate(*c.domain().start())
            .and_then(|start| c.evaluate(*c.domain().end()).map(|end| [h(start), h(end)]))
    });
    let [first, second] = endpoints;
    let [first, second] = [first?, second?];
    if first[0] == first[1]
        || first.iter().any(|v| !v.is_finite())
        || second.iter().any(|v| !v.is_finite())
    {
        return Ok(None);
    }
    let second = if (first[1] > first[0]) == (second[1] > second[0]) {
        curves[1].clone()
    } else {
        curves[1].reversed()?
    };
    let sections = match crate::section_basis::prepare(
        &[curves[0].clone(), second],
        MAX_TWO_VIEW_CONTROLS,
        limit("shared-basis controls", MAX_TWO_VIEW_CONTROLS),
        crate::section_basis::WeightScale::PerSection,
    ) {
        Ok(sections) => sections,
        // This is an optional acceleration. A large common basis does not
        // invalidate the original bounded profiles for the general solver.
        Err(GeometryError::CurveTwoViewsLimit { .. }) => return Ok(None),
        Err(error) => return Err(error),
    };
    let increasing = first[1] > first[0];
    for curve in &sections {
        for pair in curve.control_points().windows(2) {
            let delta = h(pair[1].point()) - h(pair[0].point());
            if delta == 0. || (delta > 0.) != increasing {
                return Ok(None);
            }
        }
    }
    let dual = b.cross(axis)?.scaled(1. / axis.dot(axis)?)?;
    let mut controls = Vec::new();
    for (p, q) in sections[0]
        .control_points()
        .iter()
        .zip(sections[1].control_points())
    {
        // Equal rational denominator and exactly equal shared-axis numerators
        // establish the same clock. Do not replace a general correspondence
        // by pairing normalized parameters merely because it samples closely.
        if p.weight().abs() != q.weight().abs()
            || axis.dot_point_difference(q.point(), p.point()) != 0.
        {
            return Ok(None);
        }
        let alpha = dual.dot_point_difference(q.point(), p.point());
        let point = p.point().translated(a.scaled(alpha)?)?;
        controls.push(WeightedPoint3::try_new(point, p.weight())?);
    }
    Ok(Some(NurbsCurve::try_new_rational(
        sections[0].degree(),
        controls,
        sections[0].knots().to_vec(),
    )?))
}
