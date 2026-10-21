//! Sparse native limit-position masks, independent of Rust geometry types.
use crate::SubDError;
use std::ffi::{c_char, c_int};
use std::ptr::NonNull;
#[derive(Clone, Debug)]
pub struct SubDWeights {
    pub row_offsets: Vec<usize>,
    pub columns: Vec<u32>,
    pub values: Vec<f64>,
}
#[repr(C)]
struct NativeWeights {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Default)]
struct View {
    vertex_count: usize,
    coefficient_count: usize,
    row_offsets: *const usize,
    columns: *const u32,
    values: *const f64,
}
unsafe extern "C" {
    fn vibo_subd_weights(
        bytes: *const u8,
        n: usize,
        out: *mut *mut NativeWeights,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_weights_view(weights: *const NativeWeights, view: *mut View) -> c_int;
    fn vibo_subd_weights_free(weights: *mut NativeWeights);
}
struct Owner(NonNull<NativeWeights>);
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe { vibo_subd_weights_free(self.0.as_ptr()) }
    }
}
pub fn subd_interpolation_weights(bytes: &[u8]) -> Result<SubDWeights, SubDError> {
    if bytes.len() <= 16 || bytes.len() > 64 * 1024 * 1024 {
        return Err(SubDError("Invalid SubD archive size".into()));
    }
    let mut out = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_weights(
            bytes.as_ptr(),
            bytes.len(),
            &mut out,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    let owner = NonNull::new(out).map(Owner);
    if status != 1 {
        let end = error.iter().position(|&v| v == 0).unwrap_or(error.len());
        return Err(SubDError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| SubDError("Missing native weights".into()))?;
    let mut view = View::default();
    if unsafe { vibo_subd_weights_view(owner.0.as_ptr(), &mut view) } != 1 {
        return Err(SubDError("Missing native weight view".into()));
    }
    // The owner keeps the nonempty native buffers alive for these synchronous copies.
    let row_offsets =
        unsafe { std::slice::from_raw_parts(view.row_offsets, view.vertex_count + 1) }.to_vec();
    let columns =
        unsafe { std::slice::from_raw_parts(view.columns, view.coefficient_count) }.to_vec();
    let values =
        unsafe { std::slice::from_raw_parts(view.values, view.coefficient_count) }.to_vec();
    Ok(SubDWeights {
        row_offsets,
        columns,
        values,
    })
}
