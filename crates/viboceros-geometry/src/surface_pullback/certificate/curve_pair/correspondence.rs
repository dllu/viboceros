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
    if !super::super::correspondence::valid(pairs) {
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
    let partitions =
        super::super::correspondence::partition(pairs, [&knots[0], &knots[1]], &mut budget)?;
    for [u, v] in partitions {
        let a = a.extract(&u[0], &u[1], &mut budget)?;
        let b = b.extract(&v[0], &v[1], &mut budget)?;
        let value = match interval::curves(&a, &b, limit, &mut coefficients, &mut budget, true)? {
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
    Ok(Some(result))
}
