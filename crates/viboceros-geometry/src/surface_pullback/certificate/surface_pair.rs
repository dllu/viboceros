//! Exact normalized tensor correspondence, including every one-sided knot limit.
use super::*;
mod mapped;
mod planar_image;
pub(super) mod tensor;
#[cfg(test)]
mod tests;

const MAX_CONTROLS: usize = 4096;

impl NurbsSurface {
    /// Bound the distance between the complete active tensor surfaces under
    /// independently affine-normalized U/V domains. `Some(bound)` proves a
    /// distance <= `limit` everywhere, including both sides of interior knots.
    /// `None` means outside the limit or unproved; it does not prove inequality.
    /// Reversed or swapped parameter correspondence must be supplied explicitly
    /// by reversing or swapping a source before this call.
    ///
    /// Uses exact rational restrictions of the original stored controls, knots
    /// and weights, with Bernstein product hulls and subdivision. Supports
    /// degrees <=16, <=4096 controls per source and independently common-sign
    /// weights, including unclamped and discontinuous tensors. Arithmetic size,
    /// work and depth have the shared surface-curve certificate limits. Budget
    /// exhaustion returns `None`. No trimming, topology, normal or injectivity
    /// claim is made. This bound also bounds the symmetric Hausdorff distance
    /// of these complete untrimmed images.
    pub fn parameter_surface_deviation_bound(
        &self,
        other: &Self,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        if !limit.is_finite() || limit < 0. {
            return Err(GeometryError::InvalidTolerance);
        }
        if [self, other].into_iter().any(|s| {
            s.degree_u() > MAX_DEGREE
                || s.degree_v() > MAX_DEGREE
                || s.control_points().len() > MAX_CONTROLS
        }) {
            return Ok(None);
        }
        // Equal stored tensors need no rational patch extraction. Positive or
        // negative common-sign weights prove the denominator never vanishes.
        // Keep mixed-sign identical tensors on the ordinary unsupported path.
        if self == other {
            let sign = self.control_points()[0].weight().signum();
            if self
                .control_points()
                .iter()
                .all(|p| p.weight() != 0. && p.weight().signum() == sign)
            {
                return Ok(Some(0.));
            }
        }
        match bound(self, other, limit, &mut Budget(MAX_WORK)) {
            Err(GeometryError::SurfaceCurveCertificateWorkLimit) => Ok(None),
            result => result,
        }
    }
}

fn bound(
    first: &NurbsSurface,
    second: &NurbsSurface,
    limit: Real,
    budget: &mut Budget,
) -> Result<Option<Real>, GeometryError> {
    let Some(mut a) = surface::Surface::with_discontinuities(first, budget, true)? else {
        return Ok(None);
    };
    let Some(mut b) = surface::Surface::with_discontinuities(second, budget, true)? else {
        return Ok(None);
    };
    let cuts: [Vec<Rational>; 2] = std::array::from_fn(|axis| {
        let mut cuts = a.normalized_cuts(axis);
        cuts.extend(b.normalized_cuts(axis));
        cuts.sort();
        cuts.dedup();
        cuts
    });
    let degrees = [
        [first.degree_u(), first.degree_v()],
        [second.degree_u(), second.degree_v()],
    ];
    let mut maximum = 0_f64;
    for u in cuts[0].windows(2) {
        for v in cuts[1].windows(2) {
            let rectangle = [[u[0].clone(), u[1].clone()], [v[0].clone(), v[1].clone()]];
            let a = a.normalized_patch(&rectangle, budget)?;
            let b = b.normalized_patch(&rectangle, budget)?;
            let (difference, degree) = tensor::difference(&a, &b, degrees, budget)?;
            let Some(value) = tensor::hull_bound(difference, degree, limit, budget)? else {
                return Ok(None);
            };
            maximum = maximum.max(value);
        }
    }
    Ok(Some(maximum))
}
