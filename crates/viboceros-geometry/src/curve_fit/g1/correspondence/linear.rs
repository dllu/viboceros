//! Reduce polynomial composition only after certifying a linear surrogate.
use super::*;

pub(super) fn certify(source: &NurbsCurve, candidate: &NurbsCurve, limit: Real) -> bool {
    // Bound the existing dense degree-reduction solve before allocating it.
    // This is an optional certificate proposal; every failure falls through
    // to the original source, with no extra closest-point queries.
    if source.degree() <= 1
        || source.degree() > 16
        || source.control_points().len() > MAX_INTERVALS + 1
    {
        return false;
    }
    let reserve = limit * 1e-6;
    if reserve <= 0. || !reserve.is_finite() || reserve >= limit {
        return false;
    }
    let Ok(proxy) = source.try_change_degree(1, false) else {
        return false;
    };
    // Affine normalization gives an onto, strictly increasing identity map
    // between the stored domains. Sampling or approximate collinearity cannot
    // establish this complete original-to-surrogate bound.
    let Ok(Some(_)) = source.parameter_curve_deviation_bound(&proxy, reserve) else {
        return false;
    };
    // Charge the entire proven reserve, so composition uses the certificate's
    // requested limit and does not depend on how tightly it reports a bound.
    let Some(remaining) = remaining_tolerance(limit, reserve) else {
        return false;
    };
    // Both proposals cover complete domains with ordered correspondences.
    // Composing them with the first bound certifies the ORIGINAL source;
    // neither the source clock nor the candidate's stored controls change.
    if matches!(
        proxy.parameter_curve_deviation_bound(candidate, remaining),
        Ok(Some(_))
    ) {
        return true;
    }
    matches!(
        proxy.monotone_projected_curve_deviation_bound(candidate, remaining),
        Ok(Some(_))
    )
}

fn remaining_tolerance(limit: Real, error: Real) -> Option<Real> {
    if !limit.is_finite() || !error.is_finite() || error < 0. || error >= limit {
        return None;
    }
    let remaining = if error == 0. {
        limit
    } else {
        (limit - error).next_down()
    };
    (remaining > 0.).then_some(remaining)
}

#[cfg(test)]
mod tests;
