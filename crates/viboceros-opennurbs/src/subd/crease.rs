//! Batched public native setters preserve dependencies between selected tags.
use super::*;

unsafe extern "C" {
    fn vibo_subd_crease(
        bytes: *const u8,
        count: usize,
        edges: *const usize,
        edge_count: usize,
        vertices: *const usize,
        vertex_count: usize,
        crease: c_int,
        changed: *mut usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Apply hard creases, or remove hard/soft creases, on immutable source ordinals.
/// The returned count records accepted native edits; zero denotes a no-op.
pub fn subd_crease_components(
    bytes: &[u8],
    edges: &[usize],
    vertices: &[usize],
    crease: bool,
) -> Result<(SubDData, usize), SubDError> {
    check_archive(bytes)?;
    if edges.len() > 400_000 || vertices.len() > 100_000 {
        return Err(SubDError("Excessive crease component selection".into()));
    }
    let mut changed = 0;
    let result = call(|out, error, capacity| unsafe {
        vibo_subd_crease(
            bytes.as_ptr(),
            bytes.len(),
            edges.as_ptr(),
            edges.len(),
            vertices.as_ptr(),
            vertices.len(),
            c_int::from(crease),
            &mut changed,
            out,
            error,
            capacity,
        )
    })?;
    Ok((result, changed))
}
