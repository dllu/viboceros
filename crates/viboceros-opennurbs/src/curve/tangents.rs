//! Public native tangent policy, including retained floating-point components.
use super::{CurveQueryError, c_char, c_int};

unsafe extern "C" {
    fn vibo_curve_tangents(
        degree: u32,
        count: usize,
        controls: *const f64,
        knot_count: usize,
        knots: *const f64,
        parameter_count: usize,
        parameters: *const f64,
        found: *mut c_int,
        tangents: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Unit tangent directions in request order. A missing tangent is `None`.
/// Controls contain Euclidean XYZ/weight; knots use the full-vector convention.
/// Evaluates an owned curve once per batch and retains native rounding, including
/// tiny components in geometrically constant coordinates. Degrees are bounded
/// by 64; controls and queries are each bounded by one million.
pub fn curve_tangents(
    degree: usize,
    controls: &[[f64; 4]],
    knots: &[f64],
    parameters: &[f64],
) -> Result<Vec<Option<[f64; 3]>>, CurveQueryError> {
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
        return Err(CurveQueryError("Invalid curve tangent query input".into()));
    }
    if knots[degree] >= knots[controls.len()]
        || parameters
            .iter()
            .any(|t| *t < knots[degree] || *t > knots[controls.len()])
    {
        return Err(CurveQueryError(
            "Curve tangent query is outside its domain".into(),
        ));
    }
    let mut tangents = vec![[0.; 3]; parameters.len()];
    let mut found = vec![0; parameters.len()];
    let mut error = [0u8; 1024];
    // All buffers remain live during the synchronous call. The native bridge
    // owns its curve and retains no Rust pointers.
    let status = unsafe {
        vibo_curve_tangents(
            degree as u32,
            controls.len(),
            controls.as_flattened().as_ptr(),
            knots.len(),
            knots.as_ptr(),
            parameters.len(),
            parameters.as_ptr(),
            found.as_mut_ptr(),
            tangents.as_flattened_mut().as_mut_ptr(),
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
    found
        .into_iter()
        .zip(tangents)
        .map(|(found, t)| match found {
            0 => Ok(None),
            1 if t.iter().all(|x| x.is_finite())
                && (t[0].hypot(t[1]).hypot(t[2]) - 1.).abs() <= 1e-12 =>
            {
                Ok(Some(t))
            }
            _ => Err(CurveQueryError(
                "Invalid native curve tangent result".into(),
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests;
