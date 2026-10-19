//! Owned polyline recognition, including higher-degree linear Bezier spans.
use super::{CurveQueryError, c_char, c_int};

unsafe extern "C" {
    fn vibo_curve_polyline_vertices(
        degree: u32,
        count: usize,
        controls: *const f64,
        knot_count: usize,
        knots: *const f64,
        retained: *mut usize,
        result: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Public IsPolyline policy. None means unrecognized. No curve fitting is used.
pub fn curve_polyline_vertices(
    degree: usize,
    controls: &[[f64; 4]],
    knots: &[f64],
) -> Result<Option<Vec<[f64; 3]>>, CurveQueryError> {
    let degree = u32::try_from(degree)
        .map_err(|_| CurveQueryError("Curve degree exceeds native range".into()))?;
    if degree == 0
        || degree as usize >= controls.len()
        || controls.len() > 1_000_000
        || controls.len().checked_add(degree as usize + 1) != Some(knots.len())
        || controls
            .as_flattened()
            .iter()
            .chain(knots)
            .any(|v| !v.is_finite())
    {
        return Err(CurveQueryError("Invalid polyline recognition input".into()));
    }
    let mut result = vec![[0.; 3]; controls.len()];
    let mut retained = 0;
    let mut error = [0u8; 1024];
    // Native writes at most input-count triples into this live owned buffer.
    let status = unsafe {
        vibo_curve_polyline_vertices(
            degree,
            controls.len(),
            controls.as_flattened().as_ptr(),
            knots.len(),
            knots.as_ptr(),
            &mut retained,
            result.as_flattened_mut().as_mut_ptr(),
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    if status != 1 {
        let end = error.iter().position(|b| *b == 0).unwrap_or(error.len());
        return Err(CurveQueryError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    if retained == 0 {
        return Ok(None);
    }
    if !(2..=controls.len()).contains(&retained)
        || result[..retained]
            .as_flattened()
            .iter()
            .any(|v| !v.is_finite())
    {
        return Err(CurveQueryError("Invalid native polyline vertices".into()));
    }
    result.truncate(retained);
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_polyline_recognition_handles_elevated_spans_and_declines_curvature() {
        let controls = [
            [0., 0., 0., 1.],
            [1., 0., 0., 1.],
            [2., 0., 0., 1.],
            [3., 0., 0., 1.],
            [3., 1., 0., 1.],
            [3., 2., 0., 1.],
            [3., 3., 0., 1.],
        ];
        let knots = [0., 0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1., 1.];
        assert_eq!(
            curve_polyline_vertices(3, &controls, &knots).unwrap(),
            Some(vec![[0., 0., 0.], [3., 0., 0.], [3., 3., 0.]])
        );
        let mut bent = controls;
        bent[1][1] = 0.1;
        assert_eq!(curve_polyline_vertices(3, &bent, &knots).unwrap(), None);
        let mut parameterized = controls;
        parameterized[1][0] = 0.1;
        assert_eq!(
            curve_polyline_vertices(3, &parameterized, &knots).unwrap(),
            None
        );
    }
    #[test]
    fn invalid_native_input_and_output_buffers_fail_with_bounded_diagnostics() {
        let controls = [[0., 0., 0., 1.], [1., 0., 0., 1.]];
        let knots = [0., 0., 1., 1.];
        for (degree, cv, knots) in [
            (0, controls.as_slice(), knots.as_slice()),
            (1, &controls[..1], knots.as_slice()),
            (1, controls.as_slice(), &knots[..3]),
        ] {
            assert!(curve_polyline_vertices(degree, cv, knots).is_err());
        }
        let mut bad = controls;
        bad[0][3] = 0.;
        assert!(curve_polyline_vertices(1, &bad, &knots).is_err());
        assert!(curve_polyline_vertices(1, &controls, &[0., 1., 0., 1.]).is_err());
        let mut message = [7u8; 3];
        let mut retained = 123;
        // Invalid pointers are deliberately rejected before any geometry read.
        let status = unsafe {
            vibo_curve_polyline_vertices(
                1,
                2,
                std::ptr::null(),
                0,
                std::ptr::null(),
                &mut retained,
                std::ptr::null_mut(),
                message.as_mut_ptr().add(1).cast(),
                1,
            )
        };
        assert_eq!((status, retained), (0, 0));
        assert_eq!(message, [7, 0, 7]);
    }
}
