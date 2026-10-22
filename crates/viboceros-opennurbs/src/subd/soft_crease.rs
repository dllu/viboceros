//! Edge percentages use the public native sharpness conversion and setters.
use super::*;

unsafe extern "C" {
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
    check_archive(bytes)?;
    if edits.len() > 400_000
        || edits.iter().any(|(_, weights)| {
            weights
                .iter()
                .any(|x| !x.is_finite() || !(0. ..=100.).contains(x))
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
        vibo_subd_soft_crease(
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
