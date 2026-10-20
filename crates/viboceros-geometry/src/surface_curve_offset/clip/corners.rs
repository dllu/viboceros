//! Shared planar offsets and conic bridges for admitted source corners.
use super::*;
use crate::{
    Brep, Curve3, CurveCurveIntersectionEvent, CurveOffsetCornerStyle, CurveSegment3, PolyCurve3,
};
#[cfg(test)]
mod tests;

pub(super) fn has_corners(
    source: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if source.has_full_multiplicity_kink(tolerance)? {
        return Ok(true);
    }
    if source.is_closed()? {
        return Ok(extend::direction_gap(
            source
                .tangent_at_on_side(*source.domain().end(), ParameterSide::Left)?
                .as_vector(),
            source
                .tangent_at_on_side(*source.domain().start(), ParameterSide::Right)?
                .as_vector(),
        )? > tolerance.angular());
    }
    Ok(false)
}

pub(super) fn offset(
    surface: &NurbsSurface,
    source: &NurbsCurve,
    distance: Real,
    tolerance: Tolerance,
    options: SurfaceCurveOffsetOptions,
) -> Result<(Vec<NurbsCurve>, usize), GeometryError> {
    let numerical = Tolerance::try_new(
        tolerance.absolute() * 0.05,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    // Original source admission precedes this path. Lift its certified UV
    // witness so an admitted normal displacement does not tilt a corner plane.
    let uv = surface.try_pullback_curve_certified(source, numerical)?;
    let projected = surface.try_pushup_curve_certified(&uv, numerical)?;
    let normal = surface.normal_at(0.5, 0.5)?;
    let offset = Curve3::NurbsCurve(projected).try_offset_with_operations(
        distance,
        normal,
        numerical,
        CurveOffsetCornerStyle::Round,
        options.curve_offset_operations,
    )?;
    // The surface-offset SDK fills an outside corner with an unweighted
    // quadratic through the two offset endpoints and their tangent apex.
    // Shared round offsets already trim inside corners and identify exactly
    // these bridge segments; their smooth NURBS side pieces remain unchanged.
    let offset = if let Curve3::PolyCurve(chain) = offset {
        let segments = chain
            .segments()
            .iter()
            .map(|segment| {
                let CurveSegment3::Arc(arc) = segment else {
                    return Ok(segment.clone());
                };
                let curve = CurveRef::Arc(arc);
                let a = curve
                    .evaluate_with_tangent_on_side(*arc.domain().start(), ParameterSide::Right)?;
                let b = curve
                    .evaluate_with_tangent_on_side(*arc.domain().end(), ParameterSide::Left)?;
                let apex = crate::curve_pair_support::supporting_directions_intersection(
                    a.point(),
                    a.tangent(),
                    b.point(),
                    b.tangent(),
                    numerical,
                )?;
                Ok(CurveSegment3::NurbsCurve(NurbsCurve::try_conic(
                    a.point(),
                    apex,
                    b.point(),
                    0.5,
                )?))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Curve3::PolyCurve(PolyCurve3::try_new(segments)?)
    } else {
        offset
    };
    let fitted = offset.as_ref().to_nurbs()?.try_reparameterized(0. ..=1.)?;
    // Corner spans have their own parameter clock. Intersect the constructed
    // locus, rather than evaluating the smooth source's geodesic sampler at
    // those parameters. Reuse the shared finite curve intersection producer.
    let bounds = Brep::try_surface_face(surface.clone(), numerical)?;
    let mut cuts = vec![0., 1.];
    for edge in bounds.edges() {
        let events = if let Some(producer) = options.curve_offset_operations {
            (producer.intersect)(&fitted, edge.curve(), numerical)?
        } else {
            fitted.intersection_events_with_curve(edge.curve(), numerical)?
        };
        for event in events {
            match event {
                CurveCurveIntersectionEvent::Point(p) => cuts.push(p.first_parameter()),
                CurveCurveIntersectionEvent::Overlap(p) => {
                    cuts.extend([*p.first_interval().start(), *p.first_interval().end()]);
                }
            }
            if cuts.len() > MAX_CLIP_CONTACTS {
                return Err(invalid("too many natural-boundary contacts"));
            }
        }
    }
    cuts.sort_by(Real::total_cmp);
    cuts.dedup();
    let mut intervals: Vec<[Real; 2]> = Vec::new();
    for pair in cuts.windows(2) {
        let t = pair[0].midpoint(pair[1]);
        if t <= pair[0] || t >= pair[1] {
            return Err(invalid("clipping interval is below parameter resolution"));
        }
        let point = fitted.evaluate(t)?;
        let (u, v) = surface.closest_parameters(point, numerical)?;
        if surface.evaluate(u, v)?.distance_to(point)? <= numerical.absolute() {
            if let Some(last) = intervals.last_mut().filter(|p| p[1] == pair[0]) {
                last[1] = pair[1];
            } else {
                intervals.push([pair[0], pair[1]]);
            }
        }
    }
    let curves = trim_intervals(&fitted, intervals)?;
    for curve in &curves {
        // Full image proofs reject missed boundary crossings and any rounded
        // stored controls that leave the original target's error allowance.
        surface.try_pullback_curve_certified(curve, numerical)?;
    }
    if options.closed_gaps == SurfaceCurveOffsetGapStyle::ConnectClosed && source.is_closed()? {
        closed_gap::connect(surface, curves, tolerance, 1.)
    } else {
        Ok((curves, 0))
    }
}
