//! Oriented volume first moments, independent of document grouping or markers.
use crate::exact_scalar::{Rational, rational, scalar};
use crate::{GeometryError, Point3, Real, require_finite};
use num_traits::Zero;

mod boundary;
mod mesh;
pub use boundary::VolumeBoundary;

/// Surface first-moment flux choice. Both vector fields have divergence equal
/// to the coordinate being integrated and agree on a closed oriented boundary.
/// They differ on open pieces; meshes continue to use exact tetrahedral cones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurfaceVolumeMoments {
    /// Radial cone density `q_j * (q dot n) / 4`.
    #[default]
    Cone,
    /// Average of three coordinate-direction antiderivatives:
    /// `(q_j * (q dot n) - q_j^2 * n_j / 2) / 3`.
    CoordinatePrimitives,
}
#[cfg(test)]
mod boundary_tests;
#[cfg(test)]
mod scalar_tests;
#[cfg(test)]
mod tests;

/// Signed volume and its signed first moments. Whole-shell reversal changes
/// volume sign, not centroid. Exact aggregation delays rounding and range loss.
#[derive(Clone, Debug, Default)]
pub struct VolumeMassProperties {
    volume: Rational,
    first: [Rational; 3],
}

impl VolumeMassProperties {
    /// Adds signed mass, preserving cancellation between oppositely wound shells.
    pub fn add(&mut self, other: &Self) {
        self.volume += &other.volume;
        for (sum, value) in self.first.iter_mut().zip(&other.first) {
            *sum += value;
        }
    }

    /// Whether the unrounded signed volume is exactly zero.
    pub fn is_zero(&self) -> bool {
        self.volume.is_zero()
    }

    pub fn signed_volume(&self) -> Result<Real, GeometryError> {
        scalar(&self.volume)
    }

    /// Zero signed volume has no volume centroid.
    pub fn centroid(&self) -> Result<Point3, GeometryError> {
        if self.volume.is_zero() {
            return Err(GeometryError::Degenerate {
                context: "volume centroid",
            });
        }
        Point3::try_from([
            scalar(&(&self.first[0] / &self.volume))?,
            scalar(&(&self.first[1] / &self.volume))?,
            scalar(&(&self.first[2] / &self.volume))?,
        ])
    }

    pub(crate) fn from_local_integrals(
        origin: Point3,
        scale: Real,
        values: [Real; 4],
    ) -> Result<Self, GeometryError> {
        require_finite(values, "volume moment integrals")?;
        let scale = rational(scale);
        let cubed = &scale * &scale * &scale;
        let volume = rational(values[0]) * &cubed;
        let first = std::array::from_fn(|i| {
            rational(origin.to_array()[i]) * &volume + rational(values[i + 1]) * &cubed * &scale
        });
        Ok(Self { volume, first })
    }
}
