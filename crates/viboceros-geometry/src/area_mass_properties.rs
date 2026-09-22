//! Area-weighted first moments, separate from document grouping and markers.
use crate::exact_scalar::{Rational, rational, scalar};
use crate::{GeometryError, Point3, Real, require_finite};
use num_traits::Zero;

mod mesh;
mod products;
pub(crate) mod surface;
#[cfg(test)]
mod tests;

/// Surface area and first moments. Combining properties retains unrounded
/// weighted coordinates, including when an intermediate area does not fit in
/// binary64. Quadrature and square roots remain numerical approximations.
#[derive(Clone, Debug, Default)]
pub struct AreaMassProperties {
    area: Rational,
    first: [Rational; 3],
}

impl AreaMassProperties {
    /// Adds another independently computed area distribution.
    pub fn add(&mut self, other: &Self) {
        self.area += &other.area;
        for (sum, value) in self.first.iter_mut().zip(&other.first) {
            *sum += value;
        }
    }

    /// Nonnegative surface area; fails if its rounded value is infinite.
    pub fn area(&self) -> Result<Real, GeometryError> {
        scalar(&self.area)
    }

    /// Area centroid, independent of surface/mesh winding. An empty or
    /// zero-area distribution has no centroid.
    pub fn centroid(&self) -> Result<Point3, GeometryError> {
        if self.area.is_zero() {
            return Err(GeometryError::Degenerate {
                context: "area centroid",
            });
        }
        Point3::try_from([
            scalar(&(&self.first[0] / &self.area))?,
            scalar(&(&self.first[1] / &self.area))?,
            scalar(&(&self.first[2] / &self.area))?,
        ])
    }

    pub(crate) fn at_point(area: Rational, point: Point3) -> Self {
        let first = point.to_array().map(|x| rational(x) * &area);
        Self { area, first }
    }

    pub(crate) fn from_local_integrals(
        origin: Point3,
        scale: Real,
        integrals: [Real; 4],
    ) -> Result<Self, GeometryError> {
        require_finite(integrals, "area moment integrals")?;
        if integrals[0] <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "area centroid",
            });
        }
        let scale = rational(scale);
        let squared = &scale * &scale;
        let area = rational(integrals[0]) * &squared;
        let first = std::array::from_fn(|i| {
            rational(origin.to_array()[i]) * &area + rational(integrals[i + 1]) * &squared * &scale
        });
        Ok(Self { area, first })
    }
}
