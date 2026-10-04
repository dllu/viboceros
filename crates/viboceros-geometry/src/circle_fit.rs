//! Circle fitting: PCA plane, spatial radial residuals, and bounded refinement.
use crate::{Circle3, FiniteSum, Frame3, GeometryError, Point3, Real, Tolerance, Vector3};
use faer::Mat;
use nalgebra::{Matrix2, Vector2};

pub const MAX_CIRCLE_FIT_POINTS: usize = crate::MAX_PLANE_FIT_POINTS;
const ITERATIONS: usize = 256;

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
        let seed = algebraic_seed(&coordinates, sigma[1] >= 0.001 * sigma[0])?;
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

fn algebraic_seed(
    points: &[[Real; 3]],
    well_conditioned: bool,
) -> Result<Vector2<Real>, GeometryError> {
    if well_conditioned {
        // A bounded condition number permits a compact covariance solve. Thin
        // arcs retain the SVD path instead of squaring their condition number.
        let mut mean = [0.; 3];
        for p in points {
            mean[0] += p[0];
            mean[1] += p[1];
            mean[2] += p.iter().map(|v| v * v).sum::<Real>();
        }
        mean.iter_mut().for_each(|v| *v /= points.len() as Real);
        let mut matrix = Matrix2::zeros();
        let mut rhs = Vector2::zeros();
        for p in points {
            let d = Vector2::new(p[0] - mean[0], p[1] - mean[1]);
            matrix += d * d.transpose();
            rhs += d * ((p.iter().map(|v| v * v).sum::<Real>() - mean[2]) * 0.5);
        }
        return matrix
            .lu()
            .solve(&rhs)
            .filter(|v| v.iter().all(|x| x.is_finite()))
            .ok_or(GeometryError::CircleFitDidNotConverge);
    }
    let design = Mat::from_fn(
        points.len(),
        3,
        |r, c| if c < 2 { 2. * points[r][c] } else { 1. },
    );
    let seed = design
        .thin_svd()
        .map_err(|_| GeometryError::CircleFitDidNotConverge)?;
    let values = seed.S().column_vector();
    if (0..3).any(|i| !values[i].is_finite() || values[i] <= 0.) {
        return Err(GeometryError::CircleFitDidNotConverge);
    }
    let mut solution = [0.; 3];
    for j in 0..3 {
        let dot = points
            .iter()
            .enumerate()
            .map(|(r, p)| seed.U()[(r, j)] * p.iter().map(|v| v * v).sum::<Real>())
            .sum::<Real>()
            / values[j];
        for (i, s) in solution.iter_mut().enumerate() {
            *s += seed.V()[(i, j)] * dot;
        }
    }
    Ok(Vector2::new(solution[0], solution[1]))
}

fn statistics(
    points: &[[Real; 3]],
    center: Vector2<Real>,
) -> Result<(Real, Real, Vector2<Real>, Matrix2<Real>), GeometryError> {
    let mut sum = FiniteSum::default();
    let mut derivative = Vector2::zeros();
    for p in points {
        let d = Vector2::new(center.x - p[0], center.y - p[1]);
        let distance = d.x.hypot(d.y).hypot(p[2]);
        if distance == 0. {
            return Err(GeometryError::CircleFitDidNotConverge);
        }
        sum.add(distance)?;
        derivative += d / distance;
    }
    let radius = sum.mean()?;
    derivative /= points.len() as Real;
    let mut cost = FiniteSum::default();
    let mut gradient = Vector2::zeros();
    let mut hessian = Matrix2::zeros();
    for p in points {
        let d = Vector2::new(center.x - p[0], center.y - p[1]);
        let distance = d.x.hypot(d.y).hypot(p[2]);
        let residual = distance - radius;
        let j = d / distance - derivative;
        cost.add(residual * residual)?;
        gradient += j * residual;
        hessian += j * j.transpose();
    }
    Ok((
        radius,
        cost.mean()?,
        gradient / points.len() as Real,
        hessian / points.len() as Real,
    ))
}

fn refine(
    points: &[[Real; 3]],
    mut center: Vector2<Real>,
) -> Result<(Vector2<Real>, Real), GeometryError> {
    let mut damping = 1e-6;
    let mut current = statistics(points, center)?;
    for _ in 0..ITERATIONS {
        let (radius, cost, gradient, hessian) = current;
        if gradient.norm() <= 1e-14 {
            return Ok((center, current.0));
        }
        let Some(mut step) = (hessian + Matrix2::identity() * damping)
            .lu()
            .solve(&(-gradient))
        else {
            return Err(GeometryError::CircleFitDidNotConverge);
        };
        let bound = 2. * radius.max(1.);
        if step.norm() > bound {
            step *= bound / step.norm();
        }
        let candidate = center + step;
        if candidate.iter().any(|v| !v.is_finite()) {
            return Err(GeometryError::CircleFitDidNotConverge);
        }
        let proposed = statistics(points, candidate)?;
        if proposed.1 < cost {
            center = candidate;
            current = proposed;
            damping = (damping * 0.2).max(1e-16);
            if step.norm() <= 1e-13 * (1. + center.norm()) {
                return Ok((center, current.0));
            }
        } else {
            damping *= 10.;
            if damping > 1e12 {
                return Ok((center, current.0));
            }
        }
    }
    Err(GeometryError::CircleFitDidNotConverge)
}

#[cfg(test)]
mod tests;
