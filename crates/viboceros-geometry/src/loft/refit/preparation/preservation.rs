//! Compose the original source proof with a certified shared-basis edit.
use super::*;

pub(super) fn within_remaining_deviation(
    prior: &NurbsCurve,
    chart: &NurbsCurve,
    matched: &NurbsCurve,
    deviation: Real,
    tolerance: Real,
) -> Result<bool, GeometryError> {
    if !deviation.is_finite() || deviation < 0. || deviation >= tolerance {
        return Ok(false);
    }
    // Round toward the smaller allowance. Even an error below half an ulp
    // must not disappear when subtracting from the total tolerance.
    let remaining = if deviation == 0. {
        tolerance
    } else {
        (tolerance - deviation).next_down()
    };
    if remaining <= 0.
        || prior.degree() != chart.degree()
        || prior.knots().len() != chart.knots().len()
    {
        return Ok(false);
    }
    let a = prior.domain();
    let b = chart.domain();
    let mut pairs = Vec::new();
    for (&s, &t) in prior.knots()[prior.degree()..=prior.control_points().len()]
        .iter()
        .zip(&chart.knots()[chart.degree()..=chart.control_points().len()])
    {
        let pair = [
            crate::nurbs::interval_fraction(s, *a.start(), *a.end())?,
            crate::nurbs::interval_fraction(t, *b.start(), *b.end())?,
        ];
        if pairs.last() == Some(&pair) {
            continue;
        }
        if pairs
            .last()
            .is_some_and(|last: &[Real; 2]| (0..2).any(|i| last[i] >= pair[i]))
        {
            return Ok(false);
        }
        pairs.push(pair);
    }
    // The chart supplies correspondence proposals only. This proves the full
    // prior-to-FINAL image bound, including every newly inserted knot and any
    // rounded controls/end-weight changes. Both domains are covered, so the
    // triangle inequality retains a complete symmetric source-image bound.
    match prior.mapped_curve_deviation_bound(matched, &pairs, remaining) {
        Ok(bound) => Ok(bound.is_some()),
        Err(GeometryError::CurveApproximationLimit) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
