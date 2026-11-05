//! Full public support frame, retaining closed-curve axis-flip conventions.
use super::*;
unsafe extern "C" {
    fn vibo_curve_plane_frame(
        degree: u32,
        count: usize,
        controls: *const f64,
        knot_count: usize,
        knots: *const f64,
        tolerance: f64,
        planar: *mut c_int,
        frame: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn curve_plane_frame(
    degree: usize,
    controls: &[[f64; 4]],
    knots: &[f64],
    tolerance: f64,
) -> Result<Option<[f64; 12]>, CurveQueryError> {
    if degree == 0
        || degree >= controls.len()
        || degree > u32::MAX as usize
        || controls.len() > i32::MAX as usize
        || controls.len().checked_add(degree + 1) != Some(knots.len())
        || !tolerance.is_finite()
        || tolerance < 0.
    {
        return Err(CurveQueryError(
            "Invalid curve plane frame query dimensions or tolerance".into(),
        ));
    }
    let mut frame = [0.; 12];
    let mut planar = -1;
    let mut error = [0u8; 1024];
    // Rust owns every input/output slice throughout this synchronous query.
    // The bridge constructs and drops its own curve and retains no pointers.
    let status = unsafe {
        vibo_curve_plane_frame(
            degree as u32,
            controls.len(),
            controls.as_flattened().as_ptr(),
            knots.len(),
            knots.as_ptr(),
            tolerance,
            &mut planar,
            frame.as_mut_ptr(),
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    if status != 1 || !(0..=1).contains(&planar) {
        let end = error.iter().position(|&b| b == 0).unwrap_or(error.len());
        return Err(CurveQueryError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    if planar == 0 {
        return Ok(None);
    }
    if frame.iter().any(|x| !x.is_finite()) {
        return Err(CurveQueryError("Nonfinite curve plane frame".into()));
    }
    Ok(Some(frame))
}
