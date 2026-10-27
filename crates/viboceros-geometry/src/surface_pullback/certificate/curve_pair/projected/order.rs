//! A strict parameter-order proof for the fitting correspondence.
use super::*;
use num_traits::Signed;

pub(super) fn certify(map: &NurbsCurve2) -> Result<bool, GeometryError> {
    let mut budget = Budget(MAX_WORK);
    let Some(spline) = curve::Spline::uv(map, &mut budget)? else {
        return Ok(false);
    };
    let mut cuts = spline.cuts();
    cuts.dedup();
    for pair in cuts.windows(2) {
        let net = spline.extract(&pair[0], &pair[1], &mut budget)?;
        let degree = Rational::from_integer((net.len() - 1).into());
        let coordinate = net.iter().map(|p| p[0].clone()).collect::<Vec<_>>();
        let weight = net.iter().map(|p| p[2].clone()).collect::<Vec<_>>();
        let derivative = |axis| {
            net.windows(2)
                .map(|p| &degree * (&p[1][axis] - &p[0][axis]))
                .collect::<Vec<_>>()
        };
        let left = algebra::multiply(&derivative(0), &weight, &mut budget)?;
        let right = algebra::multiply(&coordinate, &derivative(2), &mut budget)?;
        let mut numerator = Vec::with_capacity(left.len());
        for (left, right) in left.into_iter().zip(right) {
            let value = left - right;
            budget.check(&value)?;
            numerator.push([value]);
        }
        // u' = (A' W - A W') / W². Common-sign nonzero weights keep
        // W² positive. Nonnegative Bernstein coefficients with at least one
        // positive coefficient make u' strictly positive inside this span.
        // Exact subdivision can retain isolated stationary points. The
        // already-checked C0 knots join these increasing intervals.
        if !increasing(numerator, &mut budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn increasing(net: Net<1>, budget: &mut Budget) -> Result<bool, GeometryError> {
    let mut pending = vec![(net, 0)];
    while let Some((net, depth)) = pending.pop() {
        budget.charge(net.len())?;
        if net[0][0].is_negative() || net.last().unwrap()[0].is_negative() {
            return Ok(false);
        }
        if net.iter().all(|p| !p[0].is_negative()) {
            if net.iter().any(|p| p[0].is_positive()) {
                continue;
            }
            return Ok(false);
        }
        if depth == MAX_DEPTH {
            return Ok(false);
        }
        let (left, right) = algebra::split(&net, budget)?;
        pending.push((right, depth + 1));
        pending.push((left, depth + 1));
    }
    Ok(true)
}
