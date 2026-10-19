//! Public OpenNURBS recognition, independent of command/document policy.
use super::{CurveQueryError, c_char, c_int};
unsafe extern "C" {
    fn vibo_curve_analytic_form(
        degree: u32,
        count: usize,
        controls: *const f64,
        knot_count: usize,
        knots: *const f64,
        tolerance: f64,
        kind: *mut c_int,
        result: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
#[derive(Clone, Debug, PartialEq)]
pub enum AnalyticCurve {
    Line {
        start: [f64; 3],
        end: [f64; 3],
    },
    Arc {
        center: [f64; 3],
        x: [f64; 3],
        normal: [f64; 3],
        radius: f64,
        angles: [f64; 2],
    },
}
pub fn curve_analytic_form(
    degree: usize,
    controls: &[[f64; 4]],
    knots: &[f64],
    tolerance: f64,
) -> Result<Option<AnalyticCurve>, CurveQueryError> {
    let degree = u32::try_from(degree)
        .map_err(|_| CurveQueryError("Curve degree exceeds native range".into()))?;
    if degree == 0
        || degree as usize >= controls.len()
        || controls.len() > i32::MAX as usize
        || controls.len().checked_add(degree as usize + 1) != Some(knots.len())
        || !tolerance.is_finite()
        || tolerance < 0.
        || controls
            .as_flattened()
            .iter()
            .chain(knots)
            .any(|v| !v.is_finite())
    {
        return Err(CurveQueryError("Invalid analytic curve query input".into()));
    }
    let mut result = [0.; 12];
    let mut kind = -1;
    let mut error = [0_u8; 1024];
    // Declared buffer dimensions remain live for this synchronous owned query.
    let status = unsafe {
        vibo_curve_analytic_form(
            degree,
            controls.len(),
            controls.as_flattened().as_ptr(),
            knots.len(),
            knots.as_ptr(),
            tolerance,
            &mut kind,
            result.as_mut_ptr(),
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    if status != 1 || !(0..=2).contains(&kind) {
        let end = error.iter().position(|b| *b == 0).unwrap_or(error.len());
        return Err(CurveQueryError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    if result.iter().any(|v| !v.is_finite()) {
        return Err(CurveQueryError("Nonfinite analytic curve result".into()));
    }
    let triple = |i| [result[i], result[i + 1], result[i + 2]];
    Ok(match kind {
        0 => None,
        1 => Some(AnalyticCurve::Line {
            start: triple(0),
            end: triple(3),
        }),
        _ => {
            if result[9] <= 0.
                || result[10] >= result[11]
                || result[11] - result[10] > std::f64::consts::TAU + 1e-12
            {
                return Err(CurveQueryError("Invalid recognized arc".into()));
            }
            Some(AnalyticCurve::Arc {
                center: triple(0),
                x: triple(3),
                normal: triple(6),
                radius: result[9],
                angles: [result[10], result[11]],
            })
        }
    })
}

unsafe extern "C" {
    fn vibo_polyline_simplify_indices(
        count: usize,
        points: *const f64,
        tolerance: f64,
        indices: *mut usize,
        retained: *mut usize,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
/// Retained vertex indices, with the native linearity predicate and no curve
/// allocation per vertex. Open ends and the existing closed-curve seam remain.
pub fn polyline_simplify_indices(
    points: &[[f64; 3]],
    tolerance: f64,
) -> Result<Vec<usize>, CurveQueryError> {
    if !(2..=1_000_000).contains(&points.len())
        || !tolerance.is_finite()
        || tolerance < 0.
        || points.as_flattened().iter().any(|v| !v.is_finite())
    {
        return Err(CurveQueryError(
            "Invalid polyline simplification input".into(),
        ));
    }
    let mut indices = vec![0; points.len()];
    let mut retained = 0;
    let mut error = [0_u8; 1024];
    // The native stack never grows beyond the supplied input-sized index buffer.
    let status = unsafe {
        vibo_polyline_simplify_indices(
            points.len(),
            points.as_flattened().as_ptr(),
            tolerance,
            indices.as_mut_ptr(),
            &mut retained,
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
    if !(2..=points.len()).contains(&retained)
        || indices[..retained].windows(2).any(|p| p[0] >= p[1])
        || indices[0] != 0
        || indices[retained - 1] != points.len() - 1
    {
        return Err(CurveQueryError(
            "Invalid native polyline simplification indices".into(),
        ));
    }
    indices.truncate(retained);
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognition_rejects_malformed_bases_weights_and_nonfinite_queries() {
        let controls = [[0., 0., 0., 1.], [1., 0., 0., 1.]];
        let knots = [0., 0., 1., 1.];
        assert!(matches!(
            curve_analytic_form(1, &controls, &knots, 1e-8).unwrap(),
            Some(AnalyticCurve::Line { .. })
        ));
        assert!(curve_analytic_form(0, &controls, &knots, 1e-8).is_err());
        assert!(curve_analytic_form(1, &controls, &[0., 1., 0., 1.], 1e-8).is_err());
        for bad in [f64::NAN, f64::INFINITY, -1.] {
            assert!(curve_analytic_form(1, &controls, &knots, bad).is_err());
        }
        for weight in [0., f64::INFINITY, f64::NAN] {
            let mut c = controls;
            c[0][3] = weight;
            assert!(curve_analytic_form(1, &c, &knots, 1e-8).is_err());
        }
        assert!(curve_analytic_form(1, &controls, &knots[..3], 1e-8).is_err());
    }

    #[test]
    fn bulk_polyline_decisions_match_scalar_curve_recognition_and_validate_indices() {
        let points = (0..4096)
            .map(|i| [i as f64, (i / 7 % 3) as f64, 0.])
            .collect::<Vec<_>>();
        let started = std::time::Instant::now();
        let actual = polyline_simplify_indices(&points, 1e-8).unwrap();
        let bulk_ns = started.elapsed().as_nanos();
        let started = std::time::Instant::now();
        let mut expected = Vec::new();
        for i in 0..points.len() {
            while expected.len() >= 2 {
                let controls = [
                    points[expected[expected.len() - 2]],
                    points[*expected.last().unwrap()],
                    points[i],
                ]
                .map(|[x, y, z]| [x, y, z, 1.]);
                if !matches!(
                    curve_analytic_form(1, &controls, &[0., 0., 1., 2., 2.], 1e-8).unwrap(),
                    Some(AnalyticCurve::Line { .. })
                ) {
                    break;
                }
                expected.pop();
            }
            expected.push(i);
        }
        assert_eq!(actual, expected);
        eprintln!(
            "POLYLINE_SIMPLIFY_BATCH_AUDIT points={} bulk_ns={} scalar_ns={}",
            points.len(),
            bulk_ns,
            started.elapsed().as_nanos()
        );
        assert!(polyline_simplify_indices(&[], 1e-8).is_err());
        assert!(polyline_simplify_indices(&[[0., 0., 0.], [f64::NAN, 0., 0.]], 1e-8).is_err());
        assert!(polyline_simplify_indices(&[[0., 0., 0.], [1., 0., 0.]], -1.).is_err());
    }
}
