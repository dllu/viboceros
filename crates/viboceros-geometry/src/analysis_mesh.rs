//! Document analysis meshing settings, independent of color mappings.
use crate::{Real, SubDMeshDensity};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NurbsMeshDensity(Real);
impl Default for NurbsMeshDensity {
    fn default() -> Self {
        Self(0.8)
    }
}
impl NurbsMeshDensity {
    pub fn new(value: Real) -> Option<Self> {
        (value.is_finite() && (0.0..=1.0).contains(&value)).then_some(Self(value))
    }
    pub const fn value(self) -> Real {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnalysisMeshSettings {
    pub nurbs_density: NurbsMeshDensity,
    pub subd_density: SubDMeshDensity,
}
