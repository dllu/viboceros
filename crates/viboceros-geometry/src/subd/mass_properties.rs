//! Mass integration over prepared local-G1x NURBS patches, independent of display.
use super::*;
use crate::{AreaMassProperties, FiniteSum, Real, Tolerance};
impl SubD {
    /// Surface area of the local-G1x NURBS representation used for mass queries.
    /// Regular faces are exact; extraordinary neighborhoods retain the existing
    /// finite SubD-to-NURBS approximation. Display density and flat appearance
    /// do not participate. Conversion and quadrature failures return errors.
    pub fn area(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        let patches = self.to_nurbs_patches(SubDNurbsContinuity::G1x)?;
        let tolerance = patch_tolerance(tolerance, patches.len())?;
        let mut total = FiniteSum::default();
        for patch in patches {
            total.add(patch.surface.area(tolerance)?)?;
        }
        total.total()
    }
    /// Area and first moments of the same prepared representation as `area`.
    /// Exact aggregation retains weighted coordinates across patch boundaries.
    pub fn area_mass_properties(
        &self,
        tolerance: Tolerance,
    ) -> Result<AreaMassProperties, GeometryError> {
        let patches = self.to_nurbs_patches(SubDNurbsContinuity::G1x)?;
        let tolerance = patch_tolerance(tolerance, patches.len())?;
        let mut total = AreaMassProperties::default();
        for patch in patches {
            total.add(&patch.surface.area_mass_properties(tolerance)?);
        }
        Ok(total)
    }
}

// A query has one document-distance budget, shared by all prepared patches.
fn patch_tolerance(tolerance: Tolerance, count: usize) -> Result<Tolerance, GeometryError> {
    if count == 0 {
        return Err(GeometryError::Degenerate {
            context: "SubD area patches",
        });
    }
    Tolerance::try_new(
        (tolerance.absolute() / count as Real).max(Real::MIN_POSITIVE),
        tolerance.relative(),
        tolerance.angular(),
    )
}
