//! Owned native offset copies, retaining control components and authored data.
use super::*;
unsafe extern "C" {
    fn vibo_subd_offset(
        bytes: *const u8,
        count: usize,
        distance: f64,
        solid: c_int,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_offset(bytes: &[u8], distance: f64, solid: bool) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    if !distance.is_finite() || distance == 0. {
        return Err(SubDError("Invalid SubD offset distance".into()));
    }
    call(|out, error, capacity| unsafe {
        vibo_subd_offset(
            bytes.as_ptr(),
            bytes.len(),
            distance,
            c_int::from(solid),
            out,
            error,
            capacity,
        )
    })
}
