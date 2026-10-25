//! Batch public surface frames from one owned, exact NURBS definition.
use std::ffi::{c_char, c_int};
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("OpenNURBS surface: {0}")]
pub struct SurfaceQueryError(pub String);
pub struct SurfaceDefinition<'a> {
    pub degree: [u32; 2],
    pub count: [usize; 2],
    pub controls: &'a [[f64; 4]],
    pub knots: [&'a [f64]; 2],
}
unsafe extern "C" {
    fn vibo_surface_continuity_breaks(
        degree: *const u32,
        count: *const usize,
        controls: *const f64,
        knot_count: *const usize,
        knots_u: *const f64,
        knots_v: *const f64,
        regular_only: *const c_int,
        continuity: c_int,
        cos_angle_tolerance: f64,
        breaks_u: *mut f64,
        breaks_v: *mut f64,
        found: *mut usize,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_surface_frames(
        degree: *const u32,
        count: *const usize,
        controls: *const f64,
        knot_count: *const usize,
        knots_u: *const f64,
        knots_v: *const f64,
        parameter_count: usize,
        parameters: *const f64,
        found: *mut c_int,
        frames: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn surface_frames(
    surface: SurfaceDefinition<'_>,
    parameters: &[[f64; 2]],
) -> Result<Vec<Option<[f64; 12]>>, SurfaceQueryError> {
    let invalid = || SurfaceQueryError("Invalid surface frame input".into());
    if parameters.len() > 1_000_000
        || parameters.as_flattened().iter().any(|v| !v.is_finite())
        || !valid_surface(&surface)
    {
        return Err(invalid());
    }
    if parameters.is_empty() {
        return Ok(Vec::new());
    }
    let mut found = vec![0; parameters.len()];
    let mut output = vec![[0.; 12]; parameters.len()];
    let mut error = [0_u8; 1024];
    let knot_count = surface.knots.map(<[f64]>::len);
    // All buffers remain live for the synchronous call; the bridge owns its surface.
    let status = unsafe {
        vibo_surface_frames(
            surface.degree.as_ptr(),
            surface.count.as_ptr(),
            surface.controls.as_flattened().as_ptr(),
            knot_count.as_ptr(),
            surface.knots[0].as_ptr(),
            surface.knots[1].as_ptr(),
            parameters.len(),
            parameters.as_flattened().as_ptr(),
            found.as_mut_ptr(),
            output.as_flattened_mut().as_mut_ptr(),
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    if status != 1 {
        let end = error.iter().position(|b| *b == 0).unwrap_or(error.len());
        return Err(SurfaceQueryError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    output
        .into_iter()
        .zip(found)
        .map(|(values, found)| match found {
            0 => Ok(None),
            1 if values.iter().all(|v| v.is_finite()) => Ok(Some(values)),
            _ => Err(SurfaceQueryError("Invalid surface frame result".into())),
        })
        .collect()
}

fn valid_surface(surface: &SurfaceDefinition<'_>) -> bool {
    !(surface.count.contains(&0)
        || surface.count[0].checked_mul(surface.count[1]) != Some(surface.controls.len())
        || surface
            .controls
            .as_flattened()
            .iter()
            .chain(surface.knots[0])
            .chain(surface.knots[1])
            .any(|v| !v.is_finite())
        || surface.degree.contains(&0)
        || surface
            .degree
            .iter()
            .zip(surface.count)
            .any(|(d, n)| *d as usize >= n)
        || surface
            .knots
            .iter()
            .zip(surface.count)
            .zip(surface.degree)
            .any(|((k, n), d)| k.len() != n + d as usize + 1))
}
/// Public OpenNURBS G1 break parameters, using its default angular tolerance.
/// Regular-only queries omit singular transverse endpoints before traversal.
pub fn surface_discontinuities(
    surface: SurfaceDefinition<'_>,
    regular_only: [bool; 2],
) -> Result<[Vec<f64>; 2], SurfaceQueryError> {
    surface_continuity_breaks(
        surface,
        regular_only,
        crate::CurveContinuity::G1,
        1_f64.to_radians().cos(),
    )
}

/// Interior isocurve continuity breaks in either surface direction, with the
/// selected G1, G2, or Gsmooth policy and an explicit angular cosine.
pub fn surface_continuity_breaks(
    surface: SurfaceDefinition<'_>,
    regular_only: [bool; 2],
    continuity: crate::CurveContinuity,
    cos_angle_tolerance: f64,
) -> Result<[Vec<f64>; 2], SurfaceQueryError> {
    if !valid_surface(&surface)
        || !cos_angle_tolerance.is_finite()
        || !(-1. ..=1.).contains(&cos_angle_tolerance)
    {
        return Err(SurfaceQueryError(
            "Invalid surface discontinuity input".into(),
        ));
    }
    let mut result = surface.knots.map(|knots| vec![0.; knots.len()]);
    let [u, v] = &mut result;
    let mut found = [0usize; 2];
    let mut error = [0u8; 1024];
    let flags = regular_only.map(i32::from);
    let knot_count = surface.knots.map(<[f64]>::len);
    let status = unsafe {
        vibo_surface_continuity_breaks(
            surface.degree.as_ptr(),
            surface.count.as_ptr(),
            surface.controls.as_flattened().as_ptr(),
            knot_count.as_ptr(),
            surface.knots[0].as_ptr(),
            surface.knots[1].as_ptr(),
            flags.as_ptr(),
            continuity as c_int,
            cos_angle_tolerance,
            u.as_mut_ptr(),
            v.as_mut_ptr(),
            found.as_mut_ptr(),
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    if status != 1 {
        let n = error.iter().position(|&b| b == 0).unwrap_or(error.len());
        return Err(SurfaceQueryError(
            String::from_utf8_lossy(&error[..n]).into_owned(),
        ));
    }
    for (axis, values) in result.iter_mut().enumerate() {
        if found[axis] > values.len() {
            return Err(SurfaceQueryError(
                "Invalid surface discontinuity count".into(),
            ));
        }
        values.truncate(found[axis]);
        let lo = surface.knots[axis][surface.degree[axis] as usize];
        let hi = surface.knots[axis][surface.count[axis]];
        if values
            .iter()
            .any(|x| !x.is_finite() || *x <= lo || *x >= hi)
            || values.windows(2).any(|w| w[1] <= w[0])
        {
            return Err(SurfaceQueryError(
                "Invalid surface discontinuity values".into(),
            ));
        }
    }
    Ok(result)
}

/// Public native control-polygon mesh, including Greville texture coordinates
/// and evaluated normals. A fully collapsed net returns an empty mesh.
pub fn surface_control_polygon_mesh(
    surface: SurfaceDefinition<'_>,
) -> Result<crate::MeshData, SurfaceQueryError> {
    if !valid_surface(&surface)
        || surface.controls.len() > 4_000_000
        || (surface.count[0] - 1)
            .checked_mul(surface.count[1] - 1)
            .is_none_or(|n| n > 1_000_000)
    {
        return Err(SurfaceQueryError(
            "Invalid control polygon surface dimensions".into(),
        ));
    }
    unsafe extern "C" {
        fn vibo_surface_control_polygon_mesh(
            degree: *const u32,
            count: *const usize,
            controls: *const f64,
            knot_count: *const usize,
            knots_u: *const f64,
            knots_v: *const f64,
            output: *mut *mut crate::mesh::NativeResult,
            error: *mut c_char,
            capacity: usize,
        ) -> c_int;
    }
    let mut raw = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let knot_count = surface.knots.map(<[f64]>::len);
    let status = unsafe {
        vibo_surface_control_polygon_mesh(
            surface.degree.as_ptr(),
            surface.count.as_ptr(),
            surface.controls.as_flattened().as_ptr(),
            knot_count.as_ptr(),
            surface.knots[0].as_ptr(),
            surface.knots[1].as_ptr(),
            &mut raw,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    // The synchronous callback transferred ownership of this native result.
    unsafe { crate::mesh::read_mesh_result(raw, status, &error, 0) }
        .map(|r| r.mesh)
        .map_err(|e| SurfaceQueryError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_surface_angle_keeps_the_original_default_and_rejects_invalid_cosines() {
        let height = 0.25_f64.to_radians().tan();
        let controls = [0., 1.]
            .into_iter()
            .flat_map(|y| [[0., y, 0., 1.], [1., y, 0., 1.], [2., y, height, 1.]])
            .collect::<Vec<_>>();
        let u = [0., 0., 1., 2., 2.];
        let v = [0., 0., 1., 1.];
        let definition = || SurfaceDefinition {
            degree: [1, 1],
            count: [3, 2],
            controls: &controls,
            knots: [&u, &v],
        };
        assert_eq!(
            surface_discontinuities(definition(), [false; 2]).unwrap(),
            [vec![], vec![]]
        );
        for level in [
            crate::CurveContinuity::G1,
            crate::CurveContinuity::G2,
            crate::CurveContinuity::Gsmooth,
        ] {
            assert_eq!(
                surface_continuity_breaks(
                    definition(),
                    [true; 2],
                    level,
                    0.1_f64.to_radians().cos()
                )
                .unwrap(),
                [vec![1.], vec![]]
            );
        }
        for cosine in [f64::NAN, f64::INFINITY, -2., 2.] {
            assert!(
                surface_continuity_breaks(
                    definition(),
                    [true; 2],
                    crate::CurveContinuity::G1,
                    cosine
                )
                .is_err()
            );
        }
    }

    #[test]
    fn rational_arc_to_line_has_a_curvature_break_without_a_tangent_break() {
        let w = 0.5_f64.sqrt();
        let profile = [
            [0., 0., 1.],
            [1., 0., w],
            [1., 1., 1.],
            [1., 2., 1.],
            [1., 3., 1.],
        ];
        let controls = [0., 1.]
            .into_iter()
            .flat_map(|y| profile.map(|[x, z, w]| [x, y, z, w]))
            .collect::<Vec<_>>();
        let u = [0., 0., 0., 1., 1., 2., 2., 2.];
        let v = [0., 0., 1., 1.];
        let definition = || SurfaceDefinition {
            degree: [2, 1],
            count: [5, 2],
            controls: &controls,
            knots: [&u, &v],
        };
        assert_eq!(
            surface_continuity_breaks(
                definition(),
                [true; 2],
                crate::CurveContinuity::G1,
                1_f64.to_radians().cos()
            )
            .unwrap(),
            [vec![], vec![]]
        );
        for level in [crate::CurveContinuity::G2, crate::CurveContinuity::Gsmooth] {
            assert_eq!(
                surface_continuity_breaks(definition(), [true; 2], level, 1_f64.to_radians().cos())
                    .unwrap(),
                [vec![1.], vec![]]
            );
        }
    }

    #[test]
    fn warped_surface_frames_preserve_normal_when_parameter_derivative_units_diverge() {
        for model_exponent in [-20, 0, 40, 80] {
            let scale = 2_f64.powi(model_exponent);
            let controls = [
                [5., -7., 11.],
                [7., -7., 11.],
                [6., -4., 11.],
                [8., -4., 12.],
            ]
            .map(|[x, y, z]| [x * scale, y * scale, z * scale, 1.]);
            for (ue, ve) in [
                (0, 0),
                (-200, -200),
                (200, 200),
                (-600, -600),
                (600, 600),
                (-600, 600),
                (600, -600),
            ] {
                let widths = [2_f64.powi(ue), 2_f64.powi(ve)];
                let knots = widths.map(|end| [0., 0., end, end]);
                let fractions = [
                    [0., 0.],
                    [0., 1.],
                    [0.5, 0.],
                    [0.5, 1.],
                    [1., 1.],
                    [0.25, 0.625],
                ];
                let parameters = fractions.map(|[u, v]| [u * widths[0], v * widths[1]]);
                let frames = surface_frames(
                    SurfaceDefinition {
                        degree: [1, 1],
                        count: [2, 2],
                        controls: &controls,
                        knots: [&knots[0], &knots[1]],
                    },
                    &parameters,
                )
                .unwrap();
                let unit = |v: [f64; 3]| {
                    let length = v[0].hypot(v[1]).hypot(v[2]);
                    v.map(|x| x / length)
                };
                for (frame, [u, v]) in frames.into_iter().zip(fractions) {
                    let frame = frame.unwrap();
                    let origin = [5. + 2. * u + v, -7. + 3. * v, 11. + u * v];
                    let x = unit([2., 0., v]);
                    let z = unit([-3. * v, v - 2. * u, 6.]);
                    let y = [
                        z[1] * x[2] - z[2] * x[1],
                        z[2] * x[0] - z[0] * x[2],
                        z[0] * x[1] - z[1] * x[0],
                    ];
                    let label =
                        format!("model=2^{model_exponent}, domains=2^({ue},{ve}), uv=({u},{v})");
                    for i in 0..3 {
                        assert!(
                            (frame[i] / scale - origin[i]).abs() < 2e-12,
                            "{label}: origin {frame:?}"
                        );
                        for (offset, expected) in [(3, x), (6, y), (9, z)] {
                            assert!(
                                (frame[offset + i] - expected[i]).abs() < 2e-14,
                                "{label}: axes {frame:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn batch_frames_reuse_one_exact_plane_and_reject_bad_parameters_and_definitions() {
        let controls = [
            [0., 0., 5., 1.],
            [10., 0., 5., 1.],
            [0., 20., 5., 1.],
            [10., 20., 5., 1.],
        ];
        let knots = [0., 0., 1., 1.];
        let definition = || SurfaceDefinition {
            degree: [1, 1],
            count: [2, 2],
            controls: &controls,
            knots: [&knots, &knots],
        };
        let frames = surface_frames(definition(), &[[0., 0.], [0.5, 0.25], [1., 1.]]).unwrap();
        for (frame, origin) in frames
            .into_iter()
            .zip([[0., 0., 5.], [5., 5., 5.], [10., 20., 5.]])
        {
            let frame = frame.unwrap();
            assert_eq!(&frame[..3], &origin);
            assert_eq!(&frame[3..], [1., 0., 0., 0., 1., 0., 0., 0., 1.]);
        }
        assert!(surface_frames(definition(), &[[2., 0.]]).is_err());
        assert!(surface_frames(definition(), &[[f64::NAN, 0.]]).is_err());
        assert!(
            surface_frames(
                SurfaceDefinition {
                    count: [usize::MAX, 2],
                    ..definition()
                },
                &[[0., 0.]]
            )
            .is_err()
        );
        assert!(
            surface_frames(
                SurfaceDefinition {
                    degree: [0, 1],
                    ..definition()
                },
                &[[0., 0.]]
            )
            .is_err()
        );
        assert!(
            surface_frames(
                SurfaceDefinition {
                    knots: [&knots[..3], &knots],
                    ..definition()
                },
                &[[0., 0.]]
            )
            .is_err()
        );
        assert!(surface_frames(definition(), &[]).unwrap().is_empty());
    }
}
