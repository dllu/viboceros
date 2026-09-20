//! Euclidean least-squares planes, distinct from command selection policy.
use super::*;
use crate::FiniteSum;
use num_traits::{One, Signed};
mod normalized;

/// Input ceiling for a fit's linear-memory rank check and thin N-by-3 SVD.
pub const MAX_PLANE_FIT_POINTS: usize = 1_000_000;

#[cfg(test)]
mod tests;

impl PointProjection3 {
    /// Fit the plane minimizing the sum of squared orthogonal point distances.
    /// Every occurrence has equal weight. Empty input is rejected.
    ///
    /// Exactly coplanar inputs retain their exact plane, including very thin or
    /// extreme-range point sets. Coincident/collinear inputs choose one of the
    /// infinitely many containing planes. Full-dimensional data uses an
    /// isotropically scaled, exactly centered matrix and faer's thin SVD. The
    /// fitted normal is numerical, but projection preserves the exact centroid.
    /// Repeated smallest singular values admit multiple equally good normals;
    /// their choice is not a cross-engine compatibility guarantee.
    pub fn onto_best_fit_plane(points: &[Point3]) -> Result<Self, GeometryError> {
        if points.is_empty() {
            return Err(GeometryError::EmptyPointSet);
        }
        if points.len() > MAX_PLANE_FIT_POINTS {
            return Err(GeometryError::PlaneFitResourceLimit {
                maximum: MAX_PLANE_FIT_POINTS,
            });
        }
        if let Some(plane) = containing_plane(points)? {
            return Ok(plane);
        }

        let mut sums: [FiniteSum; 3] = std::array::from_fn(|_| FiniteSum::default());
        let mut lo = points[0].to_array();
        let mut hi = lo;
        for point in points {
            for (i, value) in point.to_array().into_iter().enumerate() {
                sums[i].add(value)?;
                lo[i] = lo[i].min(value);
                hi[i] = hi[i].max(value);
            }
        }
        let center = [
            sums[0].exact_mean()?,
            sums[1].exact_mean()?,
            sums[2].exact_mean()?,
        ];
        // A common scale preserves the Euclidean objective; per-axis scaling
        // would solve a different weighted least-squares problem.
        let scale = (0..3)
            .map(|i| rational(hi[i]) - rational(lo[i]))
            .max()
            .unwrap();
        let matrix = normalized::matrix(points, &center, &scale)?;
        let decomposition = matrix
            .thin_svd()
            .map_err(|_| GeometryError::PlaneFitDidNotConverge)?;
        let values = decomposition.S().column_vector();
        if (0..3).any(|i| !values[i].is_finite() || values[i] <= 0.) {
            return Err(GeometryError::PlaneFitDidNotConverge);
        }
        let index = (0..3)
            .min_by(|&i, &j| values[i].total_cmp(&values[j]))
            .unwrap();
        let normal: [f64; 3] = std::array::from_fn(|i| decomposition.V()[(i, index)]);
        crate::require_finite(normal, "best-fit plane normal")?;
        Self::new_exact(center, normal.map(rational), false)
    }
}

/// Exact affine-rank check. Full-dimensional inputs usually exit after four
/// points; planar/linear sets are completely checked, never classified by an
/// arbitrary modeling-distance threshold or a nearly singular covariance.
fn containing_plane(points: &[Point3]) -> Result<Option<PointProjection3>, GeometryError> {
    let a = points[0];
    for axis in 0..3 {
        if points
            .iter()
            .all(|point| point.to_array()[axis] == a.to_array()[axis])
        {
            return PointProjection3::onto_plane(
                a,
                Vector3::try_from(std::array::from_fn(|i| if i == axis { 1. } else { 0. }))?,
            )
            .map(Some);
        }
    }
    let Some(&b) = points.iter().find(|&&p| p != a) else {
        return PointProjection3::onto_plane(a, Vector3::try_new(0., 0., 1.)?).map(Some);
    };
    let direction = difference(b, a);
    let mut normal = None;
    for &point in points {
        let delta = difference(point, a);
        if let Some(normal) = &normal {
            let distance: Rational = delta.iter().zip(normal).map(|(a, b)| a * b).sum();
            if !distance.is_zero() {
                return Ok(None);
            }
        } else {
            let candidate = cross(&direction, &delta);
            if candidate.iter().any(|v| !v.is_zero()) {
                normal = Some(candidate);
            }
        }
    }
    let normal = normal.unwrap_or_else(|| {
        let axis = (0..3).min_by_key(|&i| direction[i].abs()).unwrap();
        let basis = std::array::from_fn(|i| {
            if i == axis {
                Rational::one()
            } else {
                Rational::zero()
            }
        });
        cross(&direction, &basis)
    });
    PointProjection3::new(a, normal, false).map(Some)
}
