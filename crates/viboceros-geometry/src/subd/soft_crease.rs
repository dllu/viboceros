//! Soft weights are separate from hard crease tags and retain edge orientation.
use super::*;

impl SubD {
    /// Convert a percentage to native 0..=4 sharpness without rounding it to a
    /// stored float before interpolation.
    pub fn soft_crease_sharpness(percentage: f64) -> Result<f64, GeometryError> {
        Ok(viboceros_opennurbs::subd_sharpness_from_percent(
            percentage,
        )?)
    }
    /// Interpolate by control-edge count through the public native factory.
    /// Inputs use native sharpness units. Outputs are stored float values;
    /// sanitization applies once to each resulting edge interval.
    pub fn interpolate_edge_sharpness(
        ends: [f64; 2],
        count: usize,
    ) -> Result<Vec<[f64; 2]>, GeometryError> {
        Ok(viboceros_opennurbs::subd_chain_sharpness(ends, count)?)
    }
    /// Apply accepted native float values, as returned by `edge_sharpness` or
    /// `interpolate_edge_sharpness`, without snapping them for a second time.
    /// Values must be exact float representations in native 0..=4 units.
    pub fn try_with_stored_edge_sharpness(
        &self,
        edits: &[(usize, [f64; 2])],
    ) -> Result<(Self, bool), GeometryError> {
        if edits.is_empty() {
            return Ok((self.clone(), false));
        }
        let (data, changed) = viboceros_opennurbs::subd_stored_sharpness(self.archive(), edits)?;
        if changed == 0 {
            Ok((self.clone(), false))
        } else {
            Self::from_native(data).map(|s| (s, true))
        }
    }
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
