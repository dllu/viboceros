//! Public native scalar conversion and edge-count interpolation.
use super::*;
unsafe extern "C" {
    fn vibo_subd_sharpness_from_percent(
        value: f64,
        out: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_chain_sharpness(
        ends: *const f64,
        count: usize,
        out: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
fn finish(status: c_int, error: &[u8]) -> Result<(), SubDError> {
    if status == 1 {
        return Ok(());
    }
    let end = error.iter().position(|&x| x == 0).unwrap_or(error.len());
    Err(SubDError(
        String::from_utf8_lossy(&error[..end]).into_owned(),
    ))
}
/// Returns a double in native sharpness units, before storage as a float.
pub fn subd_sharpness_from_percent(value: f64) -> Result<f64, SubDError> {
    let mut out = 0.;
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_sharpness_from_percent(value, &mut out, error.as_mut_ptr().cast(), error.len())
    };
    finish(status, &error)?;
    if !out.is_finite() || !(0. ..=4.).contains(&out) {
        return Err(SubDError("Invalid native sharpness conversion".into()));
    }
    Ok(out)
}
/// Input values use native 0..=4 units. Output values are accepted native floats
/// promoted to doubles. Native interpolation sanitizes each generated interval.
pub fn subd_chain_sharpness(ends: [f64; 2], count: usize) -> Result<Vec<[f64; 2]>, SubDError> {
    if !(1..=400_000).contains(&count)
        || ends
            .iter()
            .any(|x| !x.is_finite() || !(0. ..=4.).contains(x))
    {
        return Err(SubDError(
            "Invalid SubD sharpness interval or edge count".into(),
        ));
    }
    let mut out = vec![[0.; 2]; count];
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_chain_sharpness(
            ends.as_ptr(),
            count,
            out.as_flattened_mut().as_mut_ptr(),
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    finish(status, &error)?;
    if out
        .as_flattened()
        .iter()
        .any(|x| !x.is_finite() || !(0. ..=4.).contains(x) || (*x as f32 as f64) != *x)
    {
        return Err(SubDError("Invalid native chain sharpness".into()));
    }
    Ok(out)
}
