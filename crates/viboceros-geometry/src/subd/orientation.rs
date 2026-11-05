//! Retained public solid-orientation witnesses, independent of mass quadrature.
use super::*;
impl SubD {
    /// The public OpenNURBS witness captured when this immutable net was made.
    /// Unknown is preserved, rather than inferred from approximate volume.
    pub fn solid_orientation(&self) -> SubDSolidOrientation {
        self.data.solid_orientation
    }
    /// Reverse winding on a private copy, preserving the source and native IDs.
    pub fn reversed(&self) -> Result<Self, GeometryError> {
        Self::from_native(viboceros_opennurbs::subd_reverse_orientation(
            self.archive(),
        )?)
    }
}
