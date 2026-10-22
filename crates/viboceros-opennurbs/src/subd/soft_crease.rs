//! Edge percentages use the public native sharpness conversion and setters.
use super::*;

unsafe extern "C" {
    fn vibo_subd_stored_sharpness(
        bytes: *const u8,
        count: usize,
        edges: *const usize,
        weights: *const f64,
        edge_count: usize,
        changed: *mut usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_soft_crease(
        bytes: *const u8,
        count: usize,
        edges: *const usize,
        weights: *const f64,
        edge_count: usize,
        changed: *mut usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Each edge receives two endpoint weights in 0..=100 percent. Ordinals refer
/// to the immutable source view. Boundary edges and repeated ordinals decline.
pub fn subd_soft_crease(
    bytes: &[u8],
    edits: &[(usize, [f64; 2])],
) -> Result<(SubDData, usize), SubDError> {
    apply(bytes, edits, false)
}
/// Preserve accepted native float values without applying the snapping rule again.
pub fn subd_stored_sharpness(
    bytes: &[u8],
    edits: &[(usize, [f64; 2])],
) -> Result<(SubDData, usize), SubDError> {
    apply(bytes, edits, true)
}
fn apply(
    bytes: &[u8],
    edits: &[(usize, [f64; 2])],
    stored: bool,
) -> Result<(SubDData, usize), SubDError> {
    check_archive(bytes)?;
    if edits.len() > 400_000
        || edits.iter().any(|(_, weights)| {
            weights.iter().any(|x| {
                !x.is_finite()
                    || !(0. ..=if stored { 4. } else { 100. }).contains(x)
                    || stored && (*x as f32 as f64) != *x
            })
        })
    {
        return Err(SubDError("Invalid or excessive soft crease weights".into()));
    }
    let edges = edits.iter().map(|&(index, _)| index).collect::<Vec<_>>();
    let weights = edits
        .iter()
        .map(|&(_, weights)| weights)
        .collect::<Vec<_>>();
    let mut changed = 0;
    let result = call(|out, error, capacity| unsafe {
        let operation = if stored {
            vibo_subd_stored_sharpness
        } else {
            vibo_subd_soft_crease
        };
        operation(
            bytes.as_ptr(),
            bytes.len(),
            edges.as_ptr(),
            weights.as_flattened().as_ptr(),
            edits.len(),
            &mut changed,
            out,
            error,
            capacity,
        )
    })?;
    Ok((result, changed))
}
