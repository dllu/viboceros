//! Curve correspondence reuses the exact surface-image Bernstein machinery.
use super::*;

mod correspondence;
#[cfg(test)]
mod mapped_tests;
mod projected;
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
        self.mapped_curve_deviation_bound(other, &[[0., 0.], [1., 1.]], limit)
    }

    /// Certifies complete curve images using a supplied piecewise affine
    /// parameter correspondence. Each pair contains normalized parameters of
    /// `self` and `other`; pairs must increase strictly in both coordinates,
    /// start at `[0, 0]` and end at `[1, 1]`. Each domain is normalized exactly
    /// from its stored endpoints. At most 4097 pairs are accepted.
    ///
    /// A returned bound proves Euclidean distance at most `limit` everywhere
    /// along the correspondence, including both limits of every knot. Because
    /// the correspondence covers both complete domains, it also bounds their
    /// symmetric Hausdorff distance. It does not prove that this correspondence
    /// is optimal, or certify topology, tangents, or injectivity. `None` means
    /// outside this correspondence's limit or unsupported, never proof that the
    /// geometric images are different. Reverse traversal by reversing a curve.
    ///
    /// The exact extraction, weight-sign, degree, arithmetic and work limits of
    /// [`Self::parameter_curve_deviation_bound`] apply. Proposed matching pairs
    /// may come from sampling; acceptance always certifies the intervals too.
    pub fn mapped_curve_deviation_bound(
        &self,
        other: &NurbsCurve,
        correspondence: &[[Real; 2]],
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        correspondence::bound(self, other, correspondence, limit).map_err(map_work_error)
    }
}

pub(super) fn map_work_error(error: GeometryError) -> GeometryError {
    match error {
        GeometryError::SurfaceCurveCertificateWorkLimit => GeometryError::CurveApproximationLimit,
        other => other,
    }
}
