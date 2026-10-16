//! Rectangular extent policy around the shared least-squares plane fitter.
use super::*;
use crate::{Frame3, NurbsSurface, Tolerance};
#[cfg(test)]
mod tests;

impl NurbsSurface {
    /// Fit a rectangular planar surface to equally weighted spatial points.
    /// The rectangle encloses their orthogonal projections in a deterministic
    /// normal-derived frame. Its parameters measure physical edge lengths.
    /// A collapsed rectangle returns `None`; fewer than three points or an
    /// unrepresentable fit rejects. Collinear inputs use the shared fitter's
    /// deterministic containing plane. Every occurrence contributes equally.
    ///
    /// This shares the exact rank/centering and faer least-squares implementation
    /// of [`PointProjection3::onto_best_fit_plane`]. An ambiguous least-squares
    /// normal can choose another equally good plane. Surface orientation and UV
    /// direction are not guaranteed to match another engine's eigenvector sign.
    pub fn try_fit_plane(points: &[Point3]) -> Result<Option<Self>, GeometryError> {
        if points.len() < 3 {
            return Err(GeometryError::Degenerate {
                context: "plane fitting needs at least three points",
            });
        }
        if points.len() > MAX_PLANE_FIT_POINTS {
            return Err(GeometryError::PlaneFitResourceLimit {
                maximum: MAX_PLANE_FIT_POINTS,
            });
        }
        let plane = PointProjection3::onto_best_fit_plane(points)?.as_plane()?;
        let frame = Frame3::try_from_normal(
            plane.origin(),
            plane.normal().as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        // PlaneThroughPt's observed rectangular axes follow Y and -X of the
        // public normal-derived frame, rather than a minimum-area rectangle.
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for &point in points {
            let [x, y] = frame.projected_coordinates_of(point)?;
            for (axis, value) in [y, -x].into_iter().enumerate() {
                lo[axis] = lo[axis].min(value);
                hi[axis] = hi[axis].max(value);
            }
        }
        let lengths = [hi[0] - lo[0], hi[1] - lo[1]];
        crate::require_finite(lengths, "fitted plane dimensions")?;
        if lengths.iter().any(|v| *v <= 0.) {
            return Ok(None);
        }
        let corners = [
            (lo[0], lo[1]),
            (hi[0], lo[1]),
            (hi[0], hi[1]),
            (lo[0], hi[1]),
        ]
        .map(|(u, v)| frame.point_at([-v, u, 0.]));
        let [a, b, c, d] = corners;
        let [a, b, c, d] = [a?, b?, c?, d?];
        // A finite local rectangle can collapse when restored at a remote
        // origin. The existing exact plane constructor must still find an area.
        PointProjection3::onto_three_point_plane([a, b, d])?;
        let surface = Self::try_bilinear([a, b, c, d])?
            .try_reparameterized(0. ..=lengths[0], 0. ..=lengths[1])?;
        Ok(Some(surface))
    }
}
