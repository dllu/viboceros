//! Preserve authorized material regions while retaining the source geometry.
use super::*;

pub(super) fn material_groups(
    brep: &Brep,
    tolerance: Tolerance,
    index: usize,
    count: usize,
) -> Result<Option<Vec<Vec<usize>>>, StepError> {
    if let Some(order) = brep.certified_convex_solid_shell_order() {
        return Ok(Some(vec![order]));
    }
    #[cfg(feature = "native-smlib")]
    if brep.is_solid() && count > 1 {
        // Native construction is temporary. Only its material plan is used;
        // source surfaces, edges, trims and component indices remain unchanged.
        let (_, groups) = viboceros_smlib::Solid::from_brep_with_regions(brep, tolerance)
            .map_err(|error| StepError::NativeShellClassification { brep: index, error })?;
        return Ok(Some(groups));
    }
    let _ = (tolerance, index, count);
    Ok(None)
}
