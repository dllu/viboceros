//! Circle fitting: PCA plane, spatial radial residuals, and bounded refinement.
use crate::{Circle3, FiniteSum, Frame3, GeometryError, Point3, Real, Tolerance, Vector3};
use faer::Mat;
use nalgebra::Vector2;
mod distant;
mod seed;

pub const MAX_CIRCLE_FIT_POINTS: usize = crate::MAX_PLANE_FIT_POINTS;

impl Circle3 {
    /// Fit a circle on the least-squares plane through the point centroid.
    /// The radius minimizes spatial center-distance residuals, including heights
    /// above that plane. Occurrences have equal weight. Collinear/coincident
    /// sets return None. Plane basis signs are solver-dependent; callers must
    /// not assume a Rhino-compatible seam or oriented normal.
    pub fn try_fit_to_points(points: &[Point3]) -> Result<Option<Self>, GeometryError> {
        if points.len() < 3 {
            return Err(GeometryError::Degenerate {
                context: "circle fit requires at least three points",
            });
        }
        if points.len() > MAX_CIRCLE_FIT_POINTS {
            return Err(GeometryError::CircleFitResourceLimit {
                maximum: MAX_CIRCLE_FIT_POINTS,
            });
        }
        let mut sums: [FiniteSum; 3] = std::array::from_fn(|_| FiniteSum::default());
        for p in points {
            for (s, v) in sums.iter_mut().zip(p.to_array()) {
                s.add(v)?;
            }
        }
        let center = Point3::try_new(sums[0].mean()?, sums[1].mean()?, sums[2].mean()?)?;
        let deltas = points
            .iter()
            .map(|&p| center.vector_to(p).map(|v| v.to_array()))
            .collect::<Result<Vec<_>, _>>()?;
        let scale = deltas
            .iter()
            .flatten()
            .copied()
            .map(Real::abs)
            .fold(0., Real::max);
        if scale == 0. {
            return Ok(None);
        }
        let matrix = Mat::from_fn(points.len(), 3, |r, c| deltas[r][c] / scale);
        let svd = matrix
            .thin_svd()
            .map_err(|_| GeometryError::CircleFitDidNotConverge)?;
        let sigma = svd.S().column_vector();
        if (0..3).any(|i| !sigma[i].is_finite()) {
            return Err(GeometryError::CircleFitDidNotConverge);
        }
        if sigma[1] <= 16. * Real::EPSILON * sigma[0] {
            return Ok(None);
        }
        let plane = Frame3::try_from_normal(
            center,
            Vector3::try_from(std::array::from_fn(|i| svd.V()[(i, 2)]))?,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let coordinates = deltas
            .iter()
            .map(|d| {
                let d = d.map(|v| v / scale);
                [plane.x_axis(), plane.y_axis(), plane.z_axis()].map(|axis| {
                    d.iter()
                        .zip(axis.as_vector().to_array())
                        .map(|(a, b)| a * b)
                        .sum::<Real>()
                })
            })
            .collect::<Vec<_>>();
        let seed = seed::algebraic(&coordinates, sigma[1] >= 0.1 * sigma[0])?;
        let (location, radius) = refine(&coordinates, seed)?;
        let center = plane.point_at([scale * location.x, scale * location.y, 0.])?;
        Circle3::try_from_frame(
            center,
            radius * scale,
            plane.x_axis(),
            plane.z_axis(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .map(Some)
    }
}

fn refine(
    points: &[[Real; 3]],
    center: Vector2<Real>,
) -> Result<(Vector2<Real>, Real), GeometryError> {
    if center.norm() > 16. {
        return Ok((center, distant::radius(points, center)?));
    }
    crate::radial_fit::refine(
        points,
        center,
        |a, b| a.lu().solve(&b),
        || GeometryError::CircleFitDidNotConverge,
    )
}

#[cfg(test)]
mod tests;
