//! One bounded public edge spin on an owned archive copy.
use super::*;
unsafe extern "C" {
    fn vibo_subd_spin_edge(
        bytes: *const u8,
        count: usize,
        index: usize,
        clockwise: c_int,
        changed: *mut c_int,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_spin_edge(
    bytes: &[u8],
    index: usize,
    clockwise: bool,
) -> Result<(SubDData, bool), SubDError> {
    check_archive(bytes)?;
    let mut changed = 0;
    let data = call(|out, error, capacity| unsafe {
        vibo_subd_spin_edge(
            bytes.as_ptr(),
            bytes.len(),
            index,
            c_int::from(clockwise),
            &mut changed,
            out,
            error,
            capacity,
        )
    })?;
    Ok((data, changed != 0))
}
