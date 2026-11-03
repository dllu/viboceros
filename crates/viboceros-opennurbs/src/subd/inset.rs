//! Native component-preserving face insets on a private archive.
use super::*;
unsafe extern "C" {
    fn vibo_subd_inset(
        bytes: *const u8,
        count: usize,
        faces: *const usize,
        face_count: usize,
        distance: f64,
        group: c_int,
        changed: *mut usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_inset(
    bytes: &[u8],
    faces: &[usize],
    distance: f64,
    group: bool,
) -> Result<Option<SubDData>, SubDError> {
    check_archive(bytes)?;
    if faces.is_empty() || faces.len() > 100_000 || !distance.is_finite() || distance <= 0. {
        return Err(SubDError("Invalid SubD inset parameters".into()));
    }
    let mut changed = 0;
    let data = call(|out, error, capacity| unsafe {
        vibo_subd_inset(
            bytes.as_ptr(),
            bytes.len(),
            faces.as_ptr(),
            faces.len(),
            distance,
            group.into(),
            &mut changed,
            out,
            error,
            capacity,
        )
    })?;
    Ok((changed != 0).then_some(data))
}
