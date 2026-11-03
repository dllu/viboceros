//! Exact bicubic Bezier controls for faces with a regular limit neighborhood.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct SubDRegularPatch {
    pub face: usize,
    /// Sixteen Bezier controls, u varying fastest, with domain [0,1]^2.
    pub controls: [[f64; 3]; 16],
}
#[repr(C)]
struct NativePatches {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Default)]
struct PatchesView {
    patch_count: usize,
    faces: *const usize,
    controls: *const [[f64; 3]; 16],
}
unsafe extern "C" {
    fn vibo_subd_regular_patches(
        bytes: *const u8,
        count: usize,
        out: *mut *mut NativePatches,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_regular_patches_view(result: *const NativePatches, out: *mut PatchesView)
    -> c_int;
    fn vibo_subd_regular_patches_free(result: *mut NativePatches);
}
struct PatchesOwner(NonNull<NativePatches>);
impl Drop for PatchesOwner {
    fn drop(&mut self) {
        unsafe { vibo_subd_regular_patches_free(self.0.as_ptr()) }
    }
}

/// Increasing source face ordinals. Unsupported neighborhoods are omitted,
/// including extraordinary vertices and polygons that are not quads.
/// This extracts exact regular patches without approximation or source edits.
pub fn subd_regular_patches(bytes: &[u8]) -> Result<Vec<SubDRegularPatch>, SubDError> {
    check_archive(bytes)?;
    let mut out = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_regular_patches(
            bytes.as_ptr(),
            bytes.len(),
            &mut out,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    let owner = NonNull::new(out).map(PatchesOwner);
    if status != 1 {
        let end = error.iter().position(|&b| b == 0).unwrap_or(error.len());
        return Err(SubDError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| SubDError("Missing exact patch result".into()))?;
    let mut view = PatchesView::default();
    if unsafe { vibo_subd_regular_patches_view(owner.0.as_ptr(), &mut view) } != 1
        || view.patch_count > 100_000
        || (view.patch_count > 0 && (view.faces.is_null() || view.controls.is_null()))
    {
        return Err(SubDError("Invalid exact patch view".into()));
    }
    if view.patch_count == 0 {
        return Ok(Vec::new());
    }
    let faces = unsafe { std::slice::from_raw_parts(view.faces, view.patch_count) };
    let controls = unsafe { std::slice::from_raw_parts(view.controls, view.patch_count) };
    if faces.iter().any(|&i| i >= 100_000)
        || faces.windows(2).any(|w| w[0] >= w[1])
        || controls.iter().flatten().flatten().any(|v| !v.is_finite())
    {
        return Err(SubDError("Invalid exact patch layout".into()));
    }
    Ok(faces
        .iter()
        .zip(controls)
        .map(|(&face, &controls)| SubDRegularPatch { face, controls })
        .collect())
}
