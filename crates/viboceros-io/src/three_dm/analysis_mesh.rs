//! Native analysis mesh metadata, independent of document rendering state.
use super::*;

pub(super) fn analysis_nurbs_density(
    handle: &ModelHandle,
) -> Result<viboceros_geometry::NurbsMeshDensity, ThreeDmError> {
    let mut density = 0.;
    // SAFETY: the handle is live and the output pointer is writable.
    if unsafe { ffi::vibo_3dm_analysis_nurbs_density(handle.0.as_ptr(), &mut density) } == 0 {
        return Err(ThreeDmError::MalformedBridge(
            "invalid NURBS analysis density",
        ));
    }
    viboceros_geometry::NurbsMeshDensity::new(density).ok_or(ThreeDmError::MalformedBridge(
        "invalid NURBS analysis density",
    ))
}

pub(super) fn analysis_subd_density(
    handle: &ModelHandle,
) -> Result<viboceros_geometry::SubDMeshDensity, ThreeDmError> {
    let (mut level, mut adaptive) = (0, 0);
    // SAFETY: the handle is live and both output pointers are writable.
    if unsafe { ffi::vibo_3dm_analysis_subd_density(handle.0.as_ptr(), &mut level, &mut adaptive) }
        == 0
        || adaptive > 1
    {
        return Err(ThreeDmError::MalformedBridge(
            "invalid analysis SubD density",
        ));
    }
    u8::try_from(level)
        .ok()
        .and_then(|level| viboceros_geometry::SubDMeshDensity::new(level, adaptive != 0))
        .ok_or(ThreeDmError::MalformedBridge("invalid analysis SubD level"))
}
