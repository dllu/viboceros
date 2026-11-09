//! Complete tensor bounds under supplied separable piecewise affine UV maps.
use super::*;
#[cfg(test)]
mod tests;

impl NurbsSurface {
    /// Certify complete untrimmed surface images under independent, supplied
    /// piecewise affine U/V correspondences. Each map contains normalized
    /// `[self, other]` parameters, strictly increasing in both columns from
    /// `[0, 0]` to `[1, 1]`, with 2..=4097 pairs. Natural domains are normalized
    /// exactly from their stored endpoints. Reverse or swap a source explicitly.
    ///
    /// A returned bound proves distance <= `limit` everywhere along this map,
    /// including both sides of every knot. Coverage of both complete domains
    /// also proves a symmetric Hausdorff bound. Sampling may propose maps but
    /// cannot authorize a bound: all stored tensor patches are checked exactly.
    /// This does not choose an optimal map, allow coupled U/V warps, or certify
    /// trimming, topology, normals or injectivity. `None` is outside this map's
    /// limit or unproved, never proof of geometric inequality.
    ///
    /// The degree, control, weight-sign and arithmetic budgets of
    /// [`Self::parameter_surface_deviation_bound`] apply. Budget exhaustion
    /// returns `None`; invalid maps or tolerances return an error.
    pub fn mapped_surface_deviation_bound(
        &self,
        other: &Self,
        correspondence: [&[[Real; 2]]; 2],
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        if !limit.is_finite() || limit < 0. {
            return Err(GeometryError::InvalidTolerance);
        }
        validate_surface_correspondence(correspondence)?;
        if [self, other].into_iter().any(|s| {
            s.degree_u() > MAX_DEGREE
                || s.degree_v() > MAX_DEGREE
                || s.control_points().len() > MAX_CONTROLS
        }) {
            return Ok(None);
        }
        match bound(self, other, correspondence, limit, &mut Budget(MAX_WORK)) {
            Err(GeometryError::SurfaceCurveCertificateWorkLimit) => Ok(None),
            result => result,
        }
    }
}

fn bound(
    a: &NurbsSurface,
    b: &NurbsSurface,
    maps: [&[[Real; 2]]; 2],
    limit: Real,
    budget: &mut Budget,
) -> Result<Option<Real>, GeometryError> {
    let degrees = [[a.degree_u(), a.degree_v()], [b.degree_u(), b.degree_v()]];
    let Some(mut a) = surface::Surface::with_discontinuities(a, budget, true)? else {
        return Ok(None);
    };
    let Some(mut b) = surface::Surface::with_discontinuities(b, budget, true)? else {
        return Ok(None);
    };
    let mut partitions = Vec::with_capacity(2);
    for (axis, map) in maps.into_iter().enumerate() {
        let knots = [a.normalized_cuts(axis), b.normalized_cuts(axis)];
        partitions.push(correspondence::partition(
            map,
            [&knots[0], &knots[1]],
            budget,
        )?);
    }
    // Charge the entire Cartesian grid before visiting patches. A valid large
    // map cannot force an unbounded quadratic traversal or allocation.
    budget.charge(partitions[0].len().saturating_mul(partitions[1].len()))?;
    // Check attained grid points before expensive tensor products/subdivision.
    // All values are exact evaluations of the original tensors. This is only
    // rejection: passing the grid still requires every complete patch below.
    let stations: [Vec<[Rational; 2]>; 2] = std::array::from_fn(|axis| {
        std::iter::once(std::array::from_fn(|_| Rational::zero()))
            .chain(
                partitions[axis]
                    .iter()
                    .map(|interval| [interval[0][1].clone(), interval[1][1].clone()]),
            )
            .collect()
    });
    for u in &stations[0] {
        for v in &stations[1] {
            let a = a.normalized_value(&[u[0].clone(), v[0].clone()], budget)?;
            let b = b.normalized_value(&[u[1].clone(), v[1].clone()], budget)?;
            let (difference, _) = tensor::difference(&[a], &[b], [[0; 2]; 2], budget)?;
            if algebra::norm_bound(&difference[0], limit).is_none() {
                return Ok(None);
            }
        }
    }
    let mut maximum = 0_f64;
    let mut coefficients = interval::Coefficients::default();
    for u in &partitions[0] {
        for v in &partitions[1] {
            let a = a.normalized_patch(&[u[0].clone(), v[0].clone()], budget)?;
            let b = b.normalized_patch(&[u[1].clone(), v[1].clone()], budget)?;
            match interval::surfaces(&a, &b, degrees, limit, &mut coefficients, budget)? {
                interval::Outcome::Within(value) => {
                    maximum = maximum.max(value);
                    continue;
                }
                interval::Outcome::Outside => return Ok(None),
                interval::Outcome::Inconclusive => {}
            }
            let (difference, degree) = tensor::difference(&a, &b, degrees, budget)?;
            let Some(value) = tensor::hull_bound(difference, degree, limit, budget)? else {
                return Ok(None);
            };
            maximum = maximum.max(value);
        }
    }
    Ok(Some(maximum))
}
