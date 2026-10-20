//! Reconnect qualified planar gaps with shared conic and spline operations.
use super::*;
use crate::curve_pair_support::supporting_directions_intersection;

pub(super) fn connect(
    surface: &NurbsSurface,
    curves: Vec<NurbsCurve>,
    tolerance: Tolerance,
    source_period: Real,
) -> Result<(Vec<NurbsCurve>, usize), GeometryError> {
    if curves.is_empty() {
        return Ok((curves, 0));
    }
    if curves.len() == 1 {
        let c = &curves[0];
        if c.evaluate(*c.domain().start())? == c.evaluate(*c.domain().end())? {
            return Ok((curves, 0));
        }
    }
    let numerical = Tolerance::try_new(
        tolerance.absolute() * 0.01,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    surface.try_affine_patch_corners(numerical).map_err(|_| {
        invalid("closed gap reconnection is qualified for affine planar patches only")
    })?;
    let mut chain = Vec::with_capacity(2 * curves.len());
    for (i, curve) in curves.iter().enumerate() {
        chain.push(curve.clone());
        let next = &curves[(i + 1) % curves.len()];
        let end = CurveRef::NurbsCurve(curve)
            .evaluate_with_tangent_on_side(*curve.domain().end(), ParameterSide::Left)?;
        let start = CurveRef::NurbsCurve(next)
            .evaluate_with_tangent_on_side(*next.domain().start(), ParameterSide::Right)?;
        let apex = supporting_directions_intersection(
            end.point(),
            end.tangent(),
            start.point(),
            start.tangent(),
            numerical,
        )
        .map_err(|_| {
            invalid("closed gap tangent lines do not have a stable planar intersection")
        })?;
        if end
            .point()
            .vector_to(apex)?
            .dot(end.tangent().as_vector())?
            <= 0.
            || apex
                .vector_to(start.point())?
                .dot(start.tangent().as_vector())?
                <= 0.
        {
            return Err(invalid("closed gap conic would reverse a retained tangent"));
        }
        let bridge = NurbsCurve::try_conic(end.point(), apex, start.point(), 0.5)?;
        chain.push(bridge);
    }
    // Unit bridge spans match ordinary SDK charts. In an extreme chart they
    // can erase retained spans or disappear at a large offset. Retry in source
    // period units, validating the stored concatenation in model space.
    let curve =
        append(&chain, 1., tolerance).or_else(|_| append(&chain, source_period, tolerance))?;
    Ok((vec![curve], curves.len()))
}

fn append(
    chain: &[NurbsCurve],
    bridge_span: Real,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let degree = chain.iter().map(NurbsCurve::degree).max().unwrap();
    let mut joined: Option<NurbsCurve> = None;
    let mut intervals = Vec::with_capacity(chain.len());
    for (i, curve) in chain.iter().enumerate() {
        let curve = curve
            .clamped_to_active_domain()?
            .try_change_degree(degree, false)?;
        let domain = curve.domain();
        let span = if i % 2 == 1 {
            bridge_span
        } else {
            domain.end() - domain.start()
        };
        let start = joined
            .as_ref()
            .map_or(*domain.start(), |c| *c.domain().end());
        let end = start + span;
        if !span.is_finite() || span <= 0. || !end.is_finite() || end <= start {
            return Err(invalid("closed gap parameter span is unrepresentable"));
        }
        let shifted = curve.try_reparameterized(start..=end)?;
        preserve_spans(curve.knots(), shifted.knots(), degree)?;
        intervals.push([start, end]);
        joined = Some(if let Some(previous) = joined {
            previous.try_append_clamped(&shifted)?
        } else {
            shifted
        });
    }
    let joined = joined.unwrap();
    for (source, [start, end]) in chain.iter().zip(intervals) {
        for (lo, hi) in source.spans() {
            for fraction in error_fractions(16) {
                let source_t = stable_lerp(lo, hi, fraction)?;
                let expected = source.evaluate(source_t)?;
                let fraction = crate::nurbs::interval_fraction(
                    source_t,
                    *source.domain().start(),
                    *source.domain().end(),
                )?;
                let t = stable_lerp(start, end, fraction)?;
                if joined.evaluate(t)?.distance_to(expected)? > tolerance.absolute() * 0.1 {
                    return Err(invalid(
                        "closed gap parameter mapping exceeded the model tolerance",
                    ));
                }
            }
        }
    }
    Ok(joined)
}
