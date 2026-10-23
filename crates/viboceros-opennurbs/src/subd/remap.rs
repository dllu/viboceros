//! Control-corner remapping through public native expert tools.
use super::*;
unsafe extern "C" {
    fn vibo_subd_remap(
        bytes: *const u8,
        count: usize,
        copies: *const usize,
        copy_count: usize,
        retired: *const usize,
        retired_count: usize,
        edges: *const usize,
        edge_count: usize,
        corners: *const u32,
        corner_count: usize,
        weld: c_int,
        changed: *mut usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_remap_vertices(
    bytes: &[u8],
    copies: &[usize],
    retired: &[usize],
    edges: &[usize],
    faces: &[Vec<u32>],
    weld: bool,
) -> Result<(SubDData, usize), SubDError> {
    check_archive(bytes)?;
    if copies.len() > 100_000
        || retired.len() > 100_000
        || edges.len() > 400_000
        || faces.len() > 100_000
    {
        return Err(SubDError("Excessive SubD topology remap".into()));
    }
    let count = faces
        .iter()
        .try_fold(0usize, |n, f| n.checked_add(f.len()))
        .ok_or_else(|| SubDError("Excessive SubD corner count".into()))?;
    if count > 800_000 {
        return Err(SubDError("Excessive SubD corner count".into()));
    }
    let corners = faces.iter().flatten().copied().collect::<Vec<_>>();
    let mut changed = 0;
    let result = call(|out, error, capacity| unsafe {
        vibo_subd_remap(
            bytes.as_ptr(),
            bytes.len(),
            copies.as_ptr(),
            copies.len(),
            retired.as_ptr(),
            retired.len(),
            edges.as_ptr(),
            edges.len(),
            corners.as_ptr(),
            corners.len(),
            c_int::from(weld),
            &mut changed,
            out,
            error,
            capacity,
        )
    })?;
    Ok((result, changed))
}
