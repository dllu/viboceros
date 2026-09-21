//! Conservative recognition of elliptical loci from homogeneous coefficients.
use super::*;
use circularity::{binomial, unit_sphere_bound};
use nalgebra::{Matrix2, Vector2};
mod frame;
mod quadratic;
use frame::CoefficientFrame;

#[cfg(test)]
mod tests;

struct EllipticLocus {
    center: Point3,
    axes: [Vector3; 2],
    normal: Vector3,
    radii: [Real; 2],
}

/// On a sufficiently short ellipse even stable curvature jets and a circular
/// tolerance tube do not determine the actual conic center. Quadratics have an
/// algebraic center/axis proposal, so use that independent information too.
pub(super) fn quadratic_circle_consistent(
    spans: &[NurbsCurve],
    center: Point3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let Some(conic) = quadratic::proposal(spans, tolerance).ok().flatten() else {
        return Ok(false);
    };
    Ok(conic.center.distance_to(center)? <= tolerance.absolute()
        && conic
            .radii
            .iter()
            .all(|r| (r - radius).abs() <= tolerance.absolute()))
}

impl NurbsCurve {
    /// Recognizes the center of an elliptical locus, including partial arcs and
    /// circles. Does not replace the curve or assert its sweep/parameterization.
    /// Quadratics use an exact rational center formula; otherwise an affine-
    /// conditioned homogeneous coefficient system proposes the ellipse. Every original
    /// rational Bezier span must satisfy absolute plane and radial bounds after
    /// an affine map to a unit circle. Same-sign weights within each span are
    /// required. Inconclusive, degenerate or ill-conditioned cases return `None`.
    /// This is a conservative floating-point test, not an exact conic predicate.
    pub fn elliptical_center(&self, tolerance: Tolerance) -> Result<Option<Point3>, GeometryError> {
        if self.degree() < 2 {
            return Ok(None);
        }
        let spans = self.try_bezier_spans()?;
        if self.degree() == 2
            && let Some(proposal) = quadratic::proposal(&spans, tolerance).ok().flatten()
            && spans.iter().try_fold(true, |fits, span| {
                if fits {
                    proposal.contains_span(span, tolerance)
                } else {
                    Ok(false)
                }
            })?
        {
            return Ok(Some(proposal.center));
        }
        let Some(proposal) = ellipse_proposal(&spans, tolerance).ok().flatten() else {
            return Ok(None);
        };
        for span in &spans {
            if !proposal.contains_span(span, tolerance)? {
                return Ok(None);
            }
        }
        Ok(Some(proposal.center))
    }
}

/// Solve for a null vector of the Bernstein coefficients of
/// a X² + 2b XY + c Y² + 2d XW + 2e YW + f W².
/// Unlike five sampled points this also works for stationary/nonlinear rational
/// parameterizations. It is only a proposal: acceptance separately checks all
/// spans in model units, including planarity and numerical conditioning.
fn ellipse_proposal(
    spans: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<Option<EllipticLocus>, GeometryError> {
    let controls: Vec<_> = spans.iter().flat_map(|s| s.control_points()).collect();
    // Whiten the planar data before fitting its implicit conic. Isotropic
    // scaling mistakes a thin, well-defined ellipse for an unstable conic.
    // This changes only proposal coordinates, not the model-space certificate.
    let Some(frame) = CoefficientFrame::from_controls(&controls, tolerance)? else {
        return Ok(None);
    };
    let n = spans[0].degree();
    let b = binomial(n);
    let doubled = binomial(2 * n);
    // Quadratics give five equations for six coefficients. Pad a zero row so
    // the thin SVD still includes the one-dimensional nullspace.
    let mut matrix = Mat::<Real>::zeros(((2 * n + 1) * spans.len()).max(6), 6);
    // Pool all spans: a heavily refined short first span should not dictate an
    // ill-conditioned center when the rest of the curve determines it well.
    for (span_index, span) in spans.iter().enumerate() {
        let controls = span.control_points();
        let gauge = controls
            .iter()
            .map(|p| p.weight().abs())
            .fold(0., Real::max);
        let sign = controls[0].weight().signum();
        let mut q = Vec::with_capacity(controls.len());
        for control in controls {
            let w = control.weight() / gauge * sign;
            if w <= 0. {
                return Ok(None);
            }
            let v = frame.origin.vector_to(control.point())?;
            q.push([
                v.dot(frame.axes[0])? / frame.scales[0] * w,
                v.dot(frame.axes[1])? / frame.scales[1] * w,
                w,
            ]);
        }
        for (k, denominator) in doubled.iter().enumerate() {
            let row = span_index * (2 * n + 1) + k;
            for i in k.saturating_sub(n)..=k.min(n) {
                let j = k - i;
                let factor = b[i] * b[j] / denominator;
                let [xi, yi, wi] = q[i];
                let [xj, yj, wj] = q[j];
                let terms = [
                    xi * xj,
                    xi * yj + yi * xj,
                    yi * yj,
                    xi * wj + wi * xj,
                    yi * wj + wi * yj,
                    wi * wj,
                ];
                for (column, term) in terms.into_iter().enumerate() {
                    matrix[(row, column)] += factor * term;
                }
            }
            let row_scale = (0..6).map(|j| matrix[(row, j)].abs()).fold(0., Real::max);
            if !row_scale.is_finite() || row_scale == 0. {
                return Ok(None);
            }
            // Row equilibration preserves the nullspace while removing pure
            // homogeneous-weight magnitude from the conditioning estimate.
            for j in 0..6 {
                matrix[(row, j)] /= row_scale;
            }
        }
    }
    if (0..matrix.nrows()).any(|i| (0..6).any(|j| !matrix[(i, j)].is_finite())) {
        return Ok(None);
    }
    let Ok(svd) = matrix.thin_svd() else {
        return Ok(None);
    };
    let singular = svd.S().column_vector();
    let mut indices = [0, 1, 2, 3, 4, 5];
    indices.sort_by(|&a, &b| singular[a].total_cmp(&singular[b]));
    let smallest = indices[0];
    let next = singular[indices[1]];
    let largest = singular[indices[5]];
    let gap = next - singular[smallest];
    if !gap.is_finite() || gap <= 0. || !largest.is_finite() {
        return Ok(None);
    }
    let coefficients: [Real; 6] = std::array::from_fn(|i| svd.V()[(i, smallest)]);
    let [a, b, c, d, e, f] = coefficients;
    let sign = (a + c).signum();
    let quadratic = Matrix2::new(a, b, b, c) * sign;
    let linear = Vector2::new(d, e) * sign;
    let eigen = quadratic.symmetric_eigen();
    let lo = eigen.eigenvalues.min();
    let hi = eigen.eigenvalues.max();
    if !lo.is_finite() || !hi.is_finite() || lo <= 0. {
        return Ok(None);
    }
    let Some(inverse) = quadratic.try_inverse() else {
        return Ok(None);
    };
    let local_center = -(inverse * linear);
    let level = -sign * f - linear.dot(&local_center);
    if !level.is_finite() || level <= 0. {
        return Ok(None);
    }
    // Undo the affine fit coordinates, then recover orthogonal physical axes.
    // The SVD avoids subtracting nearly equal squared lengths for thin ellipses.
    let mapping = Matrix2::from_fn(|i, j| {
        frame.scales[i] * eigen.eigenvectors[(i, j)] * (level / eigen.eigenvalues[j]).sqrt()
    });
    if mapping.iter().any(|v| !v.is_finite()) {
        return Ok(None);
    }
    let physical = mapping.svd(true, false);
    let radii: [Real; 2] = physical.singular_values.into();
    let axes = physical.u.expect("requested left singular vectors");
    if radii
        .iter()
        .any(|r| !r.is_finite() || *r <= tolerance.absolute())
    {
        return Ok(None);
    }
    // A short, nearly linear arc can fit many ellipses within distance tolerance
    // while their centers differ drastically. Reject numerically unstable
    // nullspaces and positive-definite solves instead of trusting residual alone.
    // This is a conservative conditioning guard, not a certified error interval.
    // For A c = -b, perturbations satisfy
    // |dc| <= eta (sqrt(2) + 2|c|) / (lambda_min(A) - 2 eta).
    // Convert that coordinate sensitivity through the affine frame, rather
    // than penalizing the physical aspect ratio of a well-determined ellipse.
    let noise = 64. * (n + 1) as Real * Real::EPSILON * (largest / gap);
    if lo <= 2. * noise {
        return Ok(None);
    }
    let sensitivity = frame.scales[0].max(frame.scales[1])
        * noise
        * (std::f64::consts::SQRT_2 + 2. * local_center.norm())
        / (lo - 2. * noise);
    if !sensitivity.is_finite() || sensitivity > tolerance.absolute() {
        return Ok(None);
    }
    let center = frame.origin.translated(frame.vector([
        local_center.x * frame.scales[0],
        local_center.y * frame.scales[1],
    ])?)?;
    let axis = |i| -> Result<Vector3, GeometryError> {
        frame
            .vector([axes[(0, i)], axes[(1, i)]])?
            .normalized_nonzero()
            .map(|v| v.as_vector())
    };
    Ok(Some(EllipticLocus {
        center,
        axes: [axis(0)?, axis(1)?],
        normal: frame.normal,
        radii,
    }))
}

impl EllipticLocus {
    fn contains_span(
        &self,
        span: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<bool, GeometryError> {
        let controls = span.control_points();
        let gauge = controls
            .iter()
            .map(|p| p.weight().abs())
            .fold(0., Real::max);
        let sign = controls[0].weight().signum();
        let mut minimum_weight = 1.0_f64;
        let mut q = Vec::with_capacity(controls.len());
        // Split the model-space distance budget between plane and ellipse. The
        // affine inverse has operator norm max(radius), so its radial error is
        // at most that radius times the unit-circle radial error.
        let plane_tolerance = tolerance.absolute() * 0.5;
        let delta = (plane_tolerance / self.radii[0].max(self.radii[1])).min(0.25);
        for control in controls {
            let w = control.weight() / gauge * sign;
            if w <= 0. {
                return Ok(false);
            }
            minimum_weight = minimum_weight.min(w);
            let offset = self.center.vector_to(control.point())?;
            if offset.dot(self.normal)?.abs() + 32. * Real::EPSILON * offset.length()?
                > plane_tolerance
            {
                return Ok(false);
            }
            let mapped = Vector3::try_new(
                offset.dot(self.axes[0])? / self.radii[0],
                offset.dot(self.axes[1])? / self.radii[1],
                0.,
            )?;
            q.push((mapped.scaled(w)?, w));
        }
        unit_sphere_bound(&q, minimum_weight, delta)
    }
}
