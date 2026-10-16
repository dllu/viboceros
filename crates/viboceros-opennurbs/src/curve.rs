//! Owned OpenNURBS curve queries through the shared exact file constructor.
use std::ffi::{c_char, c_int};

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("OpenNURBS curve: {0}")]
pub struct CurveQueryError(pub String);

unsafe extern "C" {
    fn vibo_curve_plane_normal(
        degree: u32,
        count: usize,
        controls: *const f64,
        knot_count: usize,
        knots: *const f64,
        tolerance: f64,
        planar: *mut c_int,
        normal: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Query the oriented support plane, including OpenNURBS' line-plane and
/// closed-curve orientation conventions. `None` denotes a nonplanar curve.
/// Controls use Euclidean coordinates followed by their nonzero weight.
pub fn curve_plane_normal(
    degree: usize,
    controls: &[[f64; 4]],
    knots: &[f64],
    tolerance: f64,
) -> Result<Option<[f64; 3]>, CurveQueryError> {
    let degree = u32::try_from(degree)
        .map_err(|_| CurveQueryError("Curve degree exceeds native range".into()))?;
    if degree == 0
        || degree as usize >= controls.len()
        || controls.len() > i32::MAX as usize
        || controls.len().checked_add(degree as usize + 1) != Some(knots.len())
        || !tolerance.is_finite()
        || tolerance < 0.
    {
        return Err(CurveQueryError(
            "Invalid curve plane query dimensions or tolerance".into(),
        ));
    }
    let mut normal = [0.; 3];
    let mut planar = -1;
    let mut error = [0_u8; 1024];
    // All buffers have the declared lengths and live through the synchronous
    // call. The bridge constructs an owned curve and retains no Rust pointers.
    let status = unsafe {
        vibo_curve_plane_normal(
            degree,
            controls.len(),
            controls.as_flattened().as_ptr(),
            knots.len(),
            knots.as_ptr(),
            tolerance,
            &mut planar,
            normal.as_mut_ptr(),
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    if status != 1 || !(0..=1).contains(&planar) {
        let end = error.iter().position(|b| *b == 0).unwrap_or(error.len());
        return Err(CurveQueryError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    if planar == 0 {
        return Ok(None);
    }
    if normal.iter().any(|x| !x.is_finite()) || normal == [0.; 3] {
        return Err(CurveQueryError(
            "Nonfinite or zero curve plane normal".into(),
        ));
    }
    Ok(Some(normal))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curve_plane_query_uses_native_orientation_and_classifies_spatial_curves() {
        let line = [[0., 0., 0., 1.], [2., 3., 4., 1.]];
        let n = curve_plane_normal(1, &line, &[0., 0., 1., 1.], 1e-6)
            .unwrap()
            .unwrap();
        for (a, b) in
            n.into_iter()
                .zip([-0.9284766908852593, 0.2228344058124622, 0.297112541083283])
        {
            assert!((a - b).abs() < 1e-14);
        }
        let spatial = [
            [0., 0., 0., 1.],
            [1., 2., 1., 1.],
            [3., -1., 2., 1.],
            [4., 0., 4., 1.],
        ];
        assert_eq!(
            curve_plane_normal(3, &spatial, &[0., 0., 0., 0., 1., 1., 1., 1.], 1e-6).unwrap(),
            None
        );
    }
    #[test]
    fn curve_plane_query_rejects_invalid_inputs_before_native_evaluation() {
        let controls = [[0., 0., 0., 1.], [1., 0., 0., 1.]];
        for (degree, knots, tolerance) in [
            (0, vec![0., 0., 1., 1.], 1e-6),
            (2, vec![0., 0., 1., 1.], 1e-6),
            (1, vec![0., 1.], 1e-6),
            (1, vec![0., 0., 1., 1.], f64::NAN),
        ] {
            assert!(curve_plane_normal(degree, &controls, &knots, tolerance).is_err());
        }
        for bad in [f64::NAN, f64::INFINITY] {
            let mut invalid = controls;
            invalid[0][0] = bad;
            assert!(curve_plane_normal(1, &invalid, &[0., 0., 1., 1.], 1e-6).is_err());
        }
        assert!(curve_plane_normal(1, &controls, &[0., 1., 0., 1.], 1e-6).is_err());
        assert!(curve_plane_normal(1, &controls, &[0., 0., f64::NAN, 1.], 1e-6).is_err());
        let mut zero_weight = controls;
        zero_weight[0][3] = 0.;
        assert!(curve_plane_normal(1, &zero_weight, &[0., 0., 1., 1.], 1e-6).is_err());
    }
}
