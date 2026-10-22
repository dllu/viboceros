//! Public extrusion NURBS form, retaining path direction and individual caps.
use super::{ExtrusionError, check_archive};
use std::ffi::{c_char, c_int};
use std::ptr::NonNull;

pub struct ExtrusionSurfaceData {
    pub surface: Vec<u8>,
    pub caps: [bool; 2],
    pub path_axis: usize,
}
#[repr(C)]
struct NativeSurface {
    _opaque: [u8; 0],
}
unsafe extern "C" {
    fn vibo_extrusion_surface(
        bytes: *const u8,
        count: usize,
        out: *mut *mut NativeSurface,
        data: *mut *const u8,
        length: *mut usize,
        caps: *mut u32,
        path_axis: *mut u32,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_extrusion_surface_free(result: *mut NativeSurface);
}
struct Owner(NonNull<NativeSurface>);
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe { vibo_extrusion_surface_free(self.0.as_ptr()) }
    }
}
pub fn extrusion_surface(bytes: &[u8]) -> Result<ExtrusionSurfaceData, ExtrusionError> {
    check_archive(bytes)?;
    let (mut raw, mut data) = (std::ptr::null_mut(), std::ptr::null());
    let (mut length, mut caps, mut path_axis) = (0, 0, 0);
    let mut error = [0u8; 1024];
    // Synchronous call; native surface and transfer remain privately owned.
    let status = unsafe {
        vibo_extrusion_surface(
            bytes.as_ptr(),
            bytes.len(),
            &mut raw,
            &mut data,
            &mut length,
            &mut caps,
            &mut path_axis,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    let owner = NonNull::new(raw).map(Owner);
    if status != 1 {
        let end = error.iter().position(|&x| x == 0).unwrap_or(error.len());
        return Err(ExtrusionError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let _owner = owner.ok_or_else(|| ExtrusionError("Missing extrusion surface result".into()))?;
    if data.is_null() || !(12..=256 * 1024 * 1024).contains(&length) || caps > 3 || path_axis > 1 {
        return Err(ExtrusionError("Invalid extrusion surface transfer".into()));
    }
    Ok(ExtrusionSurfaceData {
        surface: unsafe { std::slice::from_raw_parts(data, length) }.to_vec(),
        caps: [caps & 1 != 0, caps & 2 != 0],
        path_axis: path_axis as usize,
    })
}
