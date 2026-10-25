//! Exact, monotone piecewise affine matching of two complete curve domains.
use super::*;

pub(super) fn bound(
    a: &NurbsCurve,
    b: &NurbsCurve,
    pairs: &[[Real; 2]],
    limit: Real,
) -> Result<Option<Real>, GeometryError> {
    if !limit.is_finite() || limit < 0. {
        return Err(GeometryError::InvalidTolerance);
    }
    if !(2..=4097).contains(&pairs.len())
        || pairs.first() != Some(&[0., 0.])
        || pairs.last() != Some(&[1., 1.])
        || pairs.iter().flatten().any(|x| !x.is_finite())
        || pairs.windows(2).any(|p| (0..2).any(|i| p[0][i] >= p[1][i]))
    {
        return Err(GeometryError::InvalidCurveCorrespondence);
    }
    if a.degree() > MAX_DEGREE || b.degree() > MAX_DEGREE {
        return Ok(None);
    }
    let mut budget = Budget(MAX_WORK);
    let Some(a) = curve::Spline::spatial(a, &mut budget)? else {
        return Ok(None);
    };
    let Some(b) = curve::Spline::spatial(b, &mut budget)? else {
        return Ok(None);
    };
    let knots = [a.cuts(), b.cuts()];
    let mut result = 0_f64;
    let mut coefficients = interval::Coefficients::default();
    for pair in pairs.windows(2) {
        let left = pair[0].map(rational);
        let width = std::array::from_fn::<_, 2, _>(|i| rational(pair[1][i]) - &left[i]);
        let right = pair[1].map(rational);
        let mut cuts = vec![Rational::zero(), Rational::one()];
        for i in 0..2 {
            // Only inspect knots contained in this matching interval. Repeated
            // full-order knots remain cuts, so extraction checks both limits.
            let first = knots[i].partition_point(|k| k <= &left[i]);
            for k in knots[i][first..].iter().take_while(|k| *k < &right[i]) {
                let t = (k - &left[i]) / &width[i];
                budget.check(&t)?;
                cuts.push(t);
            }
        }
        cuts.sort();
        cuts.dedup();
        for cut in cuts.windows(2) {
            let mut restrict = |spline: &curve::Spline<4>, i: usize| {
                let x = &left[i] + &cut[0] * &width[i];
                let y = &left[i] + &cut[1] * &width[i];
                budget.check(&x)?;
                budget.check(&y)?;
                spline.extract(&x, &y, &mut budget)
            };
            let a = restrict(&a, 0)?;
            let b = restrict(&b, 1)?;
            let value = match interval::curves(&a, &b, limit, &mut coefficients, &mut budget, true)?
            {
                interval::Outcome::Within(value) => value,
                interval::Outcome::Outside => return Ok(None),
                interval::Outcome::Inconclusive => {
                    let difference = algebra::difference(&a, &b, &mut budget)?;
                    let Some(value) = algebra::hull_bound(difference, limit, &mut budget)? else {
                        return Ok(None);
                    };
                    value
                }
            };
            result = result.max(value);
        }
    }
    Ok(Some(result))
}
