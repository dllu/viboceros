//! Native face subsets retain component identities and per-face properties.
use super::*;
unsafe extern "C" {
    fn vibo_subd_retain_faces(
        bytes: *const u8,
        byte_count: usize,
        indices: *const usize,
        count: usize,
        corners: *const usize,
        corner_count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_retain_faces(
    bytes: &[u8],
    indices: &[usize],
    corners: &[usize],
) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    if indices.is_empty() || indices.len() > 100_000 || corners.len() > 100_000 {
        return Err(SubDError("Invalid SubD face subset".into()));
    }
    call(|out, error, capacity| unsafe {
        vibo_subd_retain_faces(
            bytes.as_ptr(),
            bytes.len(),
            indices.as_ptr(),
            indices.len(),
            corners.as_ptr(),
            corners.len(),
            out,
            error,
            capacity,
        )
    })
}
