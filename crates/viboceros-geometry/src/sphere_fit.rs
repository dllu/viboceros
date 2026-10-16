//! Independent sphere fitting on normalized points with shared radial refinement.
use crate::{
    Circle3, FiniteSum, Frame3, GeometryError, NurbsSurface, Point3, Real, Tolerance, Vector3,
};
use faer::Mat;
use nalgebra::Vector3 as Vector;

pub const MAX_SPHERE_FIT_POINTS: usize = crate::MAX_CIRCLE_FIT_POINTS;
#[cfg(test)]
mod tests;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphereFit {
    center: Point3,
    radius: Real,
}
impl SphereFit {
    pub const fn center(self) -> Point3 {
        self.center
    }
    pub const fn radius(self) -> Real {
        self.radius
    }
    pub fn to_nurbs(self) -> Result<NurbsSurface, GeometryError> {
        NurbsSurface::try_sphere(
            Frame3::try_from_normal(
                self.center,
                Vector3::try_new(0., 0., 1.)?,
                Tolerance::NUMERICAL_VALIDATION,
            )?,
            self.radius,
        )
    }
    /// Fit radial distance residuals with equal weight for each occurrence.
    /// Collinear/coincident data return None. Coplanar inputs share the circle
    /// fitter, choosing a center in that plane. Spatial fits use a normalized
    /// algebraic least-squares seed and the existing bounded radial refiner.
    pub fn try_fit_to_points(points: &[Point3]) -> Result<Option<Self>, GeometryError> {
        if points.len() < 3 {
            return Err(GeometryError::Degenerate {
                context: "sphere fitting needs at least three points",
            });
        }
        if points.len() > MAX_SPHERE_FIT_POINTS {
            return Err(GeometryError::SphereFitResourceLimit {
                maximum: MAX_SPHERE_FIT_POINTS,
            });
        }
        let mut sums: [FiniteSum; 3] = std::array::from_fn(|_| FiniteSum::default());
        for p in points {
            for (s, v) in sums.iter_mut().zip(p.to_array()) {
                s.add(v)?;
            }
        }
        let origin = Point3::try_new(sums[0].mean()?, sums[1].mean()?, sums[2].mean()?)?;
        let mut coordinates = points
            .iter()
            .map(|&p| origin.vector_to(p).map(|v| v.to_array()))
            .collect::<Result<Vec<_>, _>>()?;
        let scale = coordinates
            .iter()
            .flatten()
            .copied()
            .map(Real::abs)
            .fold(0., Real::max);
        if scale == 0. {
            return Ok(None);
        }
        for p in &mut coordinates {
            for v in p {
                *v /= scale;
            }
        }
        let matrix = Mat::from_fn(points.len(), 3, |r, c| coordinates[r][c]);
        let svd = matrix
            .thin_svd()
            .map_err(|_| GeometryError::SphereFitDidNotConverge)?;
        let sigma = svd.S().column_vector();
        if (0..3).any(|i| !sigma[i].is_finite()) {
            return Err(GeometryError::SphereFitDidNotConverge);
        }
        if sigma[1] <= 16. * Real::EPSILON * sigma[0] {
            return Ok(None);
        }
        if points.len() == 3 || sigma[2] <= 16. * Real::EPSILON * sigma[0] {
            return Circle3::try_fit_to_points(points).map(|c| {
                c.map(|c| Self {
                    center: c.center(),
                    radius: c.radius(),
                })
            });
        }
        let design = Mat::from_fn(points.len(), 4, |r, c| {
            if c < 3 { 2. * coordinates[r][c] } else { 1. }
        });
        let squares = coordinates
            .iter()
            .map(|p| p.iter().map(|v| v * v).sum::<Real>())
            .collect::<Vec<_>>();
        let seed = design
            .thin_svd()
            .map_err(|_| GeometryError::SphereFitDidNotConverge)?;
        let values = seed.S().column_vector();
        if (0..4).any(|i| !values[i].is_finite() || values[i] <= 0.) {
            return Err(GeometryError::SphereFitDidNotConverge);
        }
        let mut solution = [0.; 4];
        for k in 0..4 {
            let mut sum = FiniteSum::default();
            for (r, &q) in squares.iter().enumerate() {
                sum.add(seed.U()[(r, k)] * q)?;
            }
            let value = sum.total()? / values[k];
            for (i, s) in solution.iter_mut().enumerate() {
                *s += seed.V()[(i, k)] * value;
            }
        }
        let location = Vector::new(solution[0], solution[1], solution[2]);
        if location.iter().any(|v| !v.is_finite()) {
            return Err(GeometryError::SphereFitDidNotConverge);
        }
        let (location, radius) = crate::radial_fit::refine(
            &coordinates,
            location,
            |a, b| a.lu().solve(&b),
            || GeometryError::SphereFitDidNotConverge,
        )?;
        let p = origin.to_array();
        let center = Point3::try_from(std::array::from_fn(|i| scale.mul_add(location[i], p[i])))?;
        let radius = radius * scale;
        if !radius.is_finite() || radius <= 0. {
            return Err(GeometryError::SphereFitDidNotConverge);
        }
        Ok(Some(Self { center, radius }))
    }
}
