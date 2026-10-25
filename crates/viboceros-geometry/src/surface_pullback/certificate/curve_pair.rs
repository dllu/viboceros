//! Curve correspondence reuses the exact surface-image Bernstein machinery.
use super::*;

#[cfg(test)]
mod tests;

impl NurbsCurve {
    /// Bounds `self(t) - other(t)` over their complete, affinely normalized
    /// domains. `Some(bound)` proves Euclidean distance at most `limit` at every
    /// parameter, including both limits of an interior knot. `None` is outside
    /// the limit or unsupported, never proof of inequality or equality.
    ///
    /// Exact rational span restrictions and Bernstein products include all
    /// stored controls, weights and knots; no sampled or rounded extraction is
    /// used. Supports degrees <=16 and independently common-sign weights.
    /// Arithmetic size, subdivision depth and work use the shared certificate
    /// limits. This proves parameter correspondence, not topology or injectivity.
    pub fn parameter_curve_deviation_bound(
        &self,
        other: &NurbsCurve,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        bound(self, other, limit).map_err(map_work_error)
    }
}

pub(super) fn map_work_error(error: GeometryError) -> GeometryError {
    match error {
        GeometryError::SurfaceCurveCertificateWorkLimit => GeometryError::CurveApproximationLimit,
        other => other,
    }
}

fn bound(a: &NurbsCurve, b: &NurbsCurve, limit: Real) -> Result<Option<Real>, GeometryError> {
    if !limit.is_finite() || limit < 0. {
        return Err(GeometryError::InvalidTolerance);
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
    let mut cuts = a.cuts();
    cuts.extend(b.cuts());
    cuts.sort();
    cuts.dedup();
    let mut bound = 0_f64;
    let mut coefficients = interval::Coefficients::default();
    for pair in cuts.windows(2) {
        let a = a.extract(&pair[0], &pair[1], &mut budget)?;
        let b = b.extract(&pair[0], &pair[1], &mut budget)?;
        match interval::curves(&a, &b, limit, &mut coefficients, &mut budget, true)? {
            interval::Outcome::Within(value) => {
                bound = bound.max(value);
                continue;
            }
            interval::Outcome::Outside => return Ok(None),
            interval::Outcome::Inconclusive => {}
        }
        let difference = algebra::difference(&a, &b, &mut budget)?;
        let Some(piece_bound) = algebra::hull_bound(difference, limit, &mut budget)? else {
            return Ok(None);
        };
        bound = bound.max(piece_bound);
    }
    Ok(Some(bound))
}
