//! Native parameter retention and document-only legacy profile migration.
use super::*;

pub(super) fn apply(
    handle: &ModelHandle,
    model: &mut ThreeDmModel,
    for_document: bool,
) -> Result<(), ThreeDmError> {
    let mut raw = ffi::ViboAnalysisParameters::default();
    // SAFETY: the live handle owns both returned parameter blocks; output is writable.
    if unsafe {
        ffi::vibo_3dm_analysis_parameters(handle.0.as_ptr(), u8::from(for_document), &mut raw)
    } == 0
        || raw.subd_adaptive > 1
        || raw.simple_planes > 1
        || (raw.data_count != 0 && raw.data.is_null())
        || raw.data_count > 65536
    {
        return Err(ThreeDmError::MalformedBridge(
            "invalid analysis mesh parameters",
        ));
    }
    model.analysis_nurbs_density = viboceros_geometry::NurbsMeshDensity::new(raw.density).ok_or(
        ThreeDmError::MalformedBridge("invalid NURBS analysis density"),
    )?;
    model.analysis_subd_density = u8::try_from(raw.subd_level)
        .ok()
        .and_then(|level| viboceros_geometry::SubDMeshDensity::new(level, raw.subd_adaptive != 0))
        .ok_or(ThreeDmError::MalformedBridge(
            "invalid analysis SubD density",
        ))?;
    model.analysis_simple_planes = raw.simple_planes != 0;
    // SAFETY: the bridge's immutable block remains live until handle destruction.
    model.analysis_mesh_parameters = (raw.data_count != 0).then(|| unsafe {
        std::sync::Arc::from(std::slice::from_raw_parts(raw.data, raw.data_count))
    });
    Ok(())
}

/// Open a document with native legacy profile migration. Raw archive APIs retain
/// the stored parameters exactly; Import keeps the destination document policy.
pub fn read_3dm_file_for_document(path: impl AsRef<Path>) -> Result<ThreeDmModel, ThreeDmError> {
    let handle = read_handle_mode(path.as_ref(), true)?;
    let units = decode_units(&handle)?;
    let tolerance = model_tolerance(&handle)?;
    let mut model = decode_model(&handle, tolerance, units, 1.)?;
    apply(&handle, &mut model, true)?;
    Ok(model)
}
