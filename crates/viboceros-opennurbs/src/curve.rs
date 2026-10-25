//! Owned OpenNURBS curve queries through the shared exact file constructor.
use std::ffi::{c_char, c_int};
mod discontinuities;
mod polyline;
pub use discontinuities::{CurveContinuity, curve_continuity_breaks, curve_discontinuities};
mod recognition;
mod speeds;
mod tangents;
pub use polyline::curve_polyline_vertices;
pub use recognition::{AnalyticCurve, curve_analytic_form, polyline_simplify_indices};
pub use speeds::curve_speeds;
pub use tangents::curve_tangents;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("OpenNURBS curve: {0}")]
pub struct CurveQueryError(pub String);

unsafe extern "C" {
    fn vibo_transform_analytic_circle(
        frame: *const f64,
        matrix: *const f64,
        result: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_arc_through_points(
        points: *const f64,
        found: *mut c_int,
        result: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
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

/// OpenNURBS' analytic circle approximation under a general affine transform.
/// Input is origin/X/Y/radius; output is origin/X/normal/radius. Nonuniform
/// placements retain a circle, whose radius follows the transformed plane area.
pub fn transform_analytic_circle(
    frame: [f64; 10],
    matrix: [[f64; 4]; 4],
) -> Result<[f64; 10], CurveQueryError> {
    if frame
        .iter()
        .chain(matrix.as_flattened())
        .any(|v| !v.is_finite())
        || frame[9] <= 0.
    {
        return Err(CurveQueryError(
            "Invalid analytic circle transform input".into(),
        ));
    }
    let mut output = [0.; 10];
    let mut error = [0_u8; 1024];
    // Fixed live buffers; the bridge transforms an owned circle synchronously.
    let status = unsafe {
        vibo_transform_analytic_circle(
            frame.as_ptr(),
            matrix.as_flattened().as_ptr(),
            output.as_mut_ptr(),
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
    if output.iter().any(|v| !v.is_finite()) || output[9] <= 0. {
        return Err(CurveQueryError(
            "Invalid transformed analytic circle".into(),
        ));
    }
    Ok(output)
}

/// Public OpenNURBS arc construction, retaining its plane and angular interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcDefinition {
    pub center: [f64; 3],
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub normal: [f64; 3],
    pub radius: f64,
    pub sweep: f64,
}
pub fn arc_through_points(points: [[f64; 3]; 3]) -> Result<Option<ArcDefinition>, CurveQueryError> {
    if points.as_flattened().iter().any(|x| !x.is_finite()) {
        return Err(CurveQueryError("Nonfinite arc construction point".into()));
    }
    let mut result = [0.; 14];
    let mut found = -1;
    let mut error = [0u8; 1024];
    // Fixed, live buffers; the synchronous bridge retains no pointers.
    let status = unsafe {
        vibo_arc_through_points(
            points.as_flattened().as_ptr(),
            &mut found,
            result.as_mut_ptr(),
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    if status != 1 || !(0..=1).contains(&found) {
        let end = error.iter().position(|b| *b == 0).unwrap_or(error.len());
        return Err(CurveQueryError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    if found == 0 {
        return Ok(None);
    }
    if result.iter().any(|x| !x.is_finite())
        || result[12] <= 0.
        || !(0. ..=std::f64::consts::TAU).contains(&result[13])
        || result[13] == 0.
    {
        return Err(CurveQueryError("Invalid native arc frame".into()));
    }
    let triple = |i| [result[i], result[i + 1], result[i + 2]];
    Ok(Some(ArcDefinition {
        center: triple(0),
        x: triple(3),
        y: triple(6),
        normal: triple(9),
        radius: result[12],
        sweep: result[13],
    }))
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
    fn analytic_circle_transform_uses_plane_area_and_rejects_invalid_buffers() {
        let frame = [0., 0., 0., 1., 0., 0., 0., 1., 0., 2.];
        let transform = [
            [-2., 0.3, 0., 7.],
            [0., 0.5, 0., 4.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        let result = transform_analytic_circle(frame, transform).unwrap();
        assert_eq!(&result[..3], &[7., 4., 0.]);
        assert_eq!(&result[3..6], &[-1., 0., 0.]);
        assert_eq!(&result[6..9], &[0., 0., -1.]);
        assert!((result[9] - 2.).abs() < 1e-14);
        for kind in 0..4 {
            let mut frame = frame;
            let mut transform = transform;
            match kind {
                0 => frame[0] = f64::NAN,
                1 => frame[9] = 0.,
                2 => transform[0][0] = f64::INFINITY,
                _ => transform[3][0] = 1.,
            }
            assert!(transform_analytic_circle(frame, transform).is_err());
        }
    }
    #[test]
    fn public_three_point_arc_returns_an_owned_plane_and_rejects_invalid_points() {
        let arc = arc_through_points([[1., 0., 0.], [0., 1., 0.], [-1., 0., 0.]])
            .unwrap()
            .unwrap();
        assert_eq!(arc.center, [0.; 3]);
        assert_eq!(arc.x, [1., 0., 0.]);
        assert_eq!(arc.y, [0., 1., 0.]);
        assert_eq!(arc.normal, [0., 0., 1.]);
        assert_eq!(arc.radius, 1.);
        assert_eq!(arc.sweep, std::f64::consts::PI);
        // A multiply/add contraction in the three-plane intersection tilted
        // this coordinate-axis frame on ARM and changed a Roadlike pole.
        let quarter = arc_through_points([
            [30., 0., 0.],
            [27.071067811865476, 7.071067811865475, 0.],
            [20., 10., 0.],
        ])
        .unwrap()
        .unwrap();
        assert_eq!(quarter.center[1], 0.);
        assert_eq!(quarter.x, [1., 0., 0.]);
        assert_eq!(quarter.y, [0., 1., 0.]);
        assert_eq!(
            arc_through_points([[0.; 3], [1., 0., 0.], [2., 0., 0.]]).unwrap(),
            None
        );
        assert!(arc_through_points([[0.; 3], [f64::NAN, 0., 0.], [2., 0., 0.]]).is_err());
    }
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
