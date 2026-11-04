//! Public native sharpness conversion, interpolation and subdivision.
use super::*;
unsafe extern "C" {
    fn vibo_subd_sharpness_refinement(
        values: *const f64,
        count: usize,
        out: *mut u32,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
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
/// Bounded subdivision information for one soft-sharp edge interval.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SubDSharpnessRefinement {
    /// Subdivisions until the half-edge adjacent to each endpoint is smooth.
    pub subdivision_counts: [u32; 2],
    break_mask: u32,
}
impl SubDSharpnessRefinement {
    /// Dyadic NURBS breaks from sharp subintervals and their one-cell halo.
    /// Values lie strictly inside the normalized source-edge domain.
    pub fn required_breaks(&self) -> impl Iterator<Item = f64> {
        let mask = self.break_mask;
        (1..32).filter_map(move |i| (mask & (1 << i) != 0).then_some(i as f64 / 32.))
    }
}
/// Public native decay, midpoint interpolation, integer snapping and float storage.
/// Inputs use 0..=4 sharpness units. Work and storage per interval are bounded.
pub fn subd_sharpness_refinement(
    values: &[[f64; 2]],
) -> Result<Vec<SubDSharpnessRefinement>, SubDError> {
    if values.len() > 400_000
        || values
            .as_flattened()
            .iter()
            .any(|s| !s.is_finite() || !(0. ..=4.).contains(s))
    {
        return Err(SubDError("Invalid SubD sharpness subdivision input".into()));
    }
    let mut out = vec![SubDSharpnessRefinement::default(); values.len()];
    if values.as_flattened().iter().all(|&s| s == 0.) {
        return Ok(out);
    }
    let mut error = [0_u8; 1024];
    let status = unsafe {
        vibo_subd_sharpness_refinement(
            values.as_flattened().as_ptr(),
            values.len(),
            out.as_mut_ptr().cast(),
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    finish(status, &error)?;
    if out
        .iter()
        .any(|v| v.subdivision_counts.iter().any(|&n| n > 4) || v.break_mask & 1 != 0)
    {
        return Err(SubDError(
            "Invalid native sharpness subdivision count".into(),
        ));
    }
    Ok(out)
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
