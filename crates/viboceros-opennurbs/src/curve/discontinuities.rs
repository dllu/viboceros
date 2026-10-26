//! Interior G1, G2, or aesthetic smoothness breaks from public OpenNURBS.
use super::{CurveQueryError, c_char, c_int};

/// Public OpenNURBS geometric continuity policies, independent of curve speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum CurveContinuity {
    G1 = 0,
    Gsmooth = 1,
    G2 = 2,
}

unsafe extern "C" {
    fn vibo_curve_discontinuities_with_curvature(
        degree: u32,
        count: usize,
        controls: *const f64,
        knot_count: usize,
        knots: *const f64,
        continuity_kind: c_int,
        cos_angle_tolerance: f64,
        curvature_tolerance: f64,
        found: *mut usize,
        parameters: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Strictly interior G1 or Gsmooth discontinuities, in increasing parameter order.
/// Controls contain Euclidean XYZ and weight. Gsmooth uses OpenNURBS' aesthetic
/// curvature policy. `cos_angle_tolerance` is the cosine of the kink angle.
/// Closed-curve seams and the original NURBS representation are retained.
pub fn curve_discontinuities(
    degree: usize,
    controls: &[[f64; 4]],
    knots: &[f64],
    smooth: bool,
    cos_angle_tolerance: f64,
) -> Result<Vec<f64>, CurveQueryError> {
    curve_continuity_breaks(
        degree,
        controls,
        knots,
        if smooth {
            CurveContinuity::Gsmooth
        } else {
            CurveContinuity::G1
        },
        cos_angle_tolerance,
    )
}

/// Interior discontinuities for the chosen G1, G2, or Gsmooth policy. G2
/// compares curvature vectors; Gsmooth uses the native aesthetic curvature test.
pub fn curve_continuity_breaks(
    degree: usize,
    controls: &[[f64; 4]],
    knots: &[f64],
    continuity: CurveContinuity,
    cos_angle_tolerance: f64,
) -> Result<Vec<f64>, CurveQueryError> {
    curve_continuity_breaks_with_curvature(
        degree,
        controls,
        knots,
        continuity,
        cos_angle_tolerance,
        f64::EPSILON.sqrt(),
    )
}

/// Geometric breaks with an explicit absolute curvature-vector tolerance.
/// Curvature has units inverse to the supplied XYZ coordinates. This retains
/// OpenNURBS' relative curvature and angular policies. Zero is permitted;
/// negative or nonfinite tolerances fail before the native call.
pub fn curve_continuity_breaks_with_curvature(
    degree: usize,
    controls: &[[f64; 4]],
    knots: &[f64],
    continuity: CurveContinuity,
    cos_angle_tolerance: f64,
    curvature_tolerance: f64,
) -> Result<Vec<f64>, CurveQueryError> {
    let degree = u32::try_from(degree)
        .map_err(|_| CurveQueryError("Curve degree exceeds native range".into()))?;
    if degree == 0
        || degree as usize >= controls.len()
        || controls.len() > 1_000_000
        || controls.len().checked_add(degree as usize + 1) != Some(knots.len())
        || !curvature_tolerance.is_finite()
        || curvature_tolerance < 0.
        || !cos_angle_tolerance.is_finite()
        || !(-1. ..=1.).contains(&cos_angle_tolerance)
    {
        return Err(CurveQueryError(
            "Invalid discontinuity query dimensions or angle".into(),
        ));
    }
    let mut parameters = vec![0.; knots.len()];
    let mut found = 0;
    let mut error = [0_u8; 1024];
    // All declared input/output buffers remain live during this synchronous
    // call; the native constructor owns its curve and retains no pointers.
    let status = unsafe {
        vibo_curve_discontinuities_with_curvature(
            degree,
            controls.len(),
            controls.as_flattened().as_ptr(),
            knots.len(),
            knots.as_ptr(),
            continuity as c_int,
            cos_angle_tolerance,
            curvature_tolerance,
            &mut found,
            parameters.as_mut_ptr(),
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    if status != 1 || found > parameters.len() {
        let end = error.iter().position(|b| *b == 0).unwrap_or(error.len());
        return Err(CurveQueryError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    parameters.truncate(found);
    let start = knots[degree as usize];
    let end = knots[controls.len()];
    if parameters
        .iter()
        .any(|p| !p.is_finite() || *p <= start || *p >= end)
        || parameters.windows(2).any(|p| p[0] >= p[1])
    {
        return Err(CurveQueryError(
            "Invalid native discontinuity parameters".into(),
        ));
    }
    Ok(parameters)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_smoothness_preserves_near_tangents_and_native_closed_seams() {
        let w = 0.5_f64.sqrt();
        let mut controls = [
            [-2., 1., 0., 1.],
            [-3., 1., 0., w],
            [-3., 0., 0., 1.],
            [-3., -1., 0., w],
            [-2., -1., 0., 1.],
            [0., -1., 0., 1.],
            [2., -1., 0., 1.],
            [3., -1., 0., w],
            [3., 0., 0., 1.],
            [3., 1., 0., w],
            [2., 1., 0., 1.],
            [0., 1., 0., 1.],
            [-2., 1., 0., 1.],
        ];
        let knots = [
            0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 5., 5., 6., 6., 6.,
        ];
        let cosine = 0.1_f64.to_radians().cos();
        for (angle, sharp, smooth) in [
            (0., vec![], vec![2., 3., 5.]),
            (0.099, vec![], vec![2., 3.]),
            (0.101, vec![5.], vec![2., 3., 5.]),
        ] {
            controls[11][1] = 1. + 2. * f64::to_radians(angle).tan();
            let before = controls;
            assert_eq!(
                curve_discontinuities(2, &controls, &knots, false, cosine).unwrap(),
                sharp
            );
            assert_eq!(
                curve_discontinuities(2, &controls, &knots, true, cosine).unwrap(),
                smooth
            );
            assert_eq!(
                curve_continuity_breaks(
                    2,
                    &controls,
                    &knots,
                    CurveContinuity::G2,
                    1_f64.to_radians().cos()
                )
                .unwrap(),
                vec![2., 3., 5.]
            );
            assert_eq!(controls, before);
        }
    }
    #[test]
    fn explicit_curvature_tolerance_retains_native_units_and_default_policy() {
        let points = [
            [0., 0., 0., 1.],
            [1. / 3., 0., 0., 1.],
            [2. / 3., 0., 0., 1.],
            [1., 0., 0., 1.],
            [4. / 3., 0., 0., 1.],
            [5. / 3., 0.001, 0., 1.],
            [2., 0.003, 0., 1.],
        ];
        let knots = [0., 0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1., 1.];
        let cosine = 1_f64.to_radians().cos();
        assert_eq!(
            curve_continuity_breaks(3, &points, &knots, CurveContinuity::G2, cosine).unwrap(),
            vec![0.5]
        );
        assert!(
            curve_continuity_breaks_with_curvature(
                3,
                &points,
                &knots,
                CurveContinuity::G2,
                cosine,
                0.01
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            curve_continuity_breaks_with_curvature(
                3,
                &points,
                &knots,
                CurveContinuity::G2,
                cosine,
                0.005
            )
            .unwrap(),
            vec![0.5]
        );
        for tolerance in [-1., f64::NAN, f64::INFINITY] {
            assert!(
                curve_continuity_breaks_with_curvature(
                    3,
                    &points,
                    &knots,
                    CurveContinuity::G2,
                    cosine,
                    tolerance
                )
                .is_err()
            );
        }
    }

    #[test]
    fn invalid_discontinuity_inputs_decline_without_native_buffer_overruns() {
        let controls = [[0., 0., 0., 1.], [1., 0., 0., 1.]];
        for (degree, knots, cosine) in [
            (0, vec![0., 0., 1., 1.], 0.),
            (2, vec![0., 0., 1., 1.], 0.),
            (1, vec![0., 1.], 0.),
            (1, vec![0., 0., 1., 1.], f64::NAN),
            (1, vec![0., 0., 1., 1.], 2.),
            (1, vec![0., f64::NAN, 1., 1.], 0.),
            (1, vec![0., 1., 0., 1.], 0.),
        ] {
            assert!(curve_discontinuities(degree, &controls, &knots, true, cosine).is_err());
        }
        let mut invalid = controls;
        invalid[1][3] = 0.;
        assert!(curve_discontinuities(1, &invalid, &[0., 0., 1., 1.], true, 0.).is_err());
    }
}
