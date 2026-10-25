//! Native derivative magnitudes for floating-point-sensitive command policies.
use super::{CurveQueryError, c_char, c_int};

unsafe extern "C" {
    fn vibo_curve_speeds(
        degree: u32,
        count: usize,
        controls: *const f64,
        knot_count: usize,
        knots: *const f64,
        parameter_count: usize,
        parameters: *const f64,
        speeds: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Derivative magnitudes in request order, including zero at stationary points.
/// Retains public OpenNURBS evaluation and vector-length rounding. Controls use
/// Euclidean XYZ/weight and full knots. Degrees are limited to 64; controls and
/// queries to one million each. Sources are owned temporarily and remain unchanged.
pub fn curve_speeds(
    degree: usize,
    controls: &[[f64; 4]],
    knots: &[f64],
    parameters: &[f64],
) -> Result<Vec<f64>, CurveQueryError> {
    if !(1..=64).contains(&degree)
        || degree >= controls.len()
        || controls.len() > 1_000_000
        || parameters.len() > 1_000_000
        || controls.len().checked_add(degree + 1) != Some(knots.len())
        || controls
            .as_flattened()
            .iter()
            .chain(knots)
            .chain(parameters)
            .any(|x| !x.is_finite())
        || controls.iter().any(|p| p[3] == 0.)
        || knots.windows(2).any(|p| p[0] > p[1])
    {
        return Err(CurveQueryError("Invalid curve speed query input".into()));
    }
    if knots[degree] >= knots[controls.len()]
        || parameters
            .iter()
            .any(|t| *t < knots[degree] || *t > knots[controls.len()])
    {
        return Err(CurveQueryError(
            "Curve speed query is outside its domain".into(),
        ));
    }
    let mut result = vec![0.; parameters.len()];
    let mut error = [0u8; 1024];
    // All buffers live through this synchronous call; native code retains no pointers.
    let status = unsafe {
        vibo_curve_speeds(
            degree as u32,
            controls.len(),
            controls.as_flattened().as_ptr(),
            knots.len(),
            knots.as_ptr(),
            parameters.len(),
            parameters.as_ptr(),
            result.as_mut_ptr(),
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
    if result.iter().any(|x| !x.is_finite() || *x < 0.) {
        return Err(CurveQueryError("Invalid native curve speed result".into()));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stationary_points_query_order_and_invalid_inputs_are_distinct() {
        let controls = [[0., 0., 0., 1.], [1., 0., 0., 1.], [0., 0., 0., 1.]];
        let knots = [0., 0., 0., 1., 1., 1.];
        let before = controls;
        assert_eq!(
            curve_speeds(2, &controls, &knots, &[1., 0.5, 0.]).unwrap(),
            vec![2., 0., 2.]
        );
        assert!(curve_speeds(2, &controls, &knots, &[]).unwrap().is_empty());
        assert_eq!(controls, before);
        for parameters in [vec![-1.], vec![2.], vec![f64::NAN]] {
            assert!(curve_speeds(2, &controls, &knots, &parameters).is_err());
        }
        assert!(curve_speeds(65, &controls, &knots, &[0.]).is_err());
        assert!(curve_speeds(2, &controls, &[0., 0., 1., 0., 1., 1.], &[0.]).is_err());
        let mut pole = controls;
        pole[1][3] = -1.;
        assert!(curve_speeds(2, &pole, &knots, &[0.5]).is_err());
        for weight in [0., f64::NAN, f64::INFINITY] {
            let mut invalid = controls;
            invalid[1][3] = weight;
            assert!(curve_speeds(2, &invalid, &knots, &[0.]).is_err());
        }
    }
}
