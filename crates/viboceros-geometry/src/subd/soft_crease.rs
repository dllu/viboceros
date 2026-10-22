//! Soft weights are separate from hard crease tags and retain edge orientation.
use super::*;

impl SubD {
    /// Replace the two endpoint weights of each selected interior control edge.
    /// Values are percentages in 0..=100; 100 remains a soft edge. The native
    /// kernel sanitizes values near subdivision integers and stores floats.
    /// All edits are validated together and leave this source unchanged.
    pub fn try_with_soft_crease_weights(
        &self,
        edits: &[(usize, [f64; 2])],
    ) -> Result<(Self, bool), GeometryError> {
        if edits.is_empty() {
            return Ok((self.clone(), false));
        }
        let (data, changed) = viboceros_opennurbs::subd_soft_crease(self.archive(), edits)?;
        if changed == 0 {
            Ok((self.clone(), false))
        } else {
            Self::from_native(data).map(|result| (result, true))
        }
    }
}
