use viboceros_geometry::{
    Curve3, CurveFlowOptions, CurveFlowPointMorph, LineSegment, Point3, PointMorph, Tolerance,
};

#[test]
fn opposite_line_flow_retains_its_half_turn_plane_under_model_scaling() {
    let mut checked = 0;
    let mut failures = Vec::new();
    for exponent in [-600, -200, -20, 0, 20, 40, 200, 600] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
        let p = |x, y, z| Point3::try_new(x * scale, y * scale, z * scale).unwrap();
        let base =
            Curve3::Line(LineSegment::try_new(p(0., 0., 0.), p(3., 0., 0.), tolerance).unwrap());
        for offset in [[5., 7., 11.], [5., -3., 4.], [5., 0., 0.]] {
            for stretch in [false, true] {
                checked += 1;
                let label = format!("scale=2^{exponent} offset={offset:?} stretch={stretch}");
                let target = Curve3::Line(
                    LineSegment::try_new(
                        p(offset[0], offset[1], offset[2]),
                        p(offset[0] - 6., offset[1], offset[2]),
                        tolerance,
                    )
                    .unwrap(),
                );
                match CurveFlowPointMorph::try_new(
                    base.clone(),
                    target,
                    CurveFlowOptions {
                        stretch,
                        ..Default::default()
                    },
                    tolerance,
                ) {
                    Ok(morph) => {
                        let n = if offset[1] == 0. && offset[2] == 0. {
                            [0., 1., 0.]
                        } else {
                            let r = offset[1].hypot(offset[2]);
                            [0., -offset[2] / r, offset[1] / r]
                        };
                        let mut failed = false;
                        for [x, y, z] in [[1., 0.2, 0.4], [0., -0.3, 0.5], [4., 0.4, -0.2]] {
                            let dot = n[1] * y + n[2] * z;
                            let expected = p(
                                offset[0] - x * if stretch { 2. } else { 1. },
                                offset[1] + 2. * n[1] * dot - y,
                                offset[2] + 2. * n[2] * dot - z,
                            );
                            let actual = morph.morph_point(p(x, y, z)).unwrap();
                            if actual.distance_to(expected).unwrap() / scale > 2e-12 {
                                failed = true;
                            }
                        }
                        if failed {
                            failures.push(format!("{label}: wrong half-turn map"));
                        }
                    }
                    Err(error) => failures.push(format!("{label}: {error:?}")),
                }
            }
        }
    }
    eprintln!(
        "FLOW_DIRECTION_UNITS cases={checked} failures={}",
        failures.len()
    );
    assert_eq!(checked, 48);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn opposite_line_flow_preserves_transverse_data_when_origin_difference_overflows() {
    let length = 2_f64.powi(977);
    let tolerance = Tolerance::try_new(1e278, 1e-30, 1e-9).unwrap();
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let base = Curve3::Line(
        LineSegment::try_new(
            p(0., -1e308, 1e-200),
            p(0., -1e308 + length, 1e-200),
            tolerance,
        )
        .unwrap(),
    );
    let target = Curve3::Line(
        LineSegment::try_new(
            p(0., 1e308, -1e-200),
            p(0., 1e308 - length, -1e-200),
            tolerance,
        )
        .unwrap(),
    );
    let morph = CurveFlowPointMorph::try_new(
        base,
        target,
        CurveFlowOptions {
            stretch: false,
            ..Default::default()
        },
        tolerance,
    )
    .unwrap();
    let mut failures = 0;
    for point in [
        p(0.2 * length, -1e308 + 0.25 * length, 0.4 * length),
        p(-0.3 * length, -1e308 + 0.75 * length, -0.2 * length),
    ] {
        let expected = p(point.x(), -point.y(), -point.z());
        let actual = morph.morph_point(point).unwrap();
        if actual != expected {
            failures += 1;
        }
    }
    eprintln!("FLOW_DIRECTION_ORIGINS cases=2 failures={failures}");
    assert_eq!(
        failures, 0,
        "overflowing origin difference erased the transverse offset"
    );
}

#[test]
fn short_line_flow_borrows_the_tilted_arc_plane_consistently() {
    use viboceros_geometry::{Circle3, CircularArc3, Vector3};
    let mut checked = 0;
    for exponent in [-600, -200, 0, 200, 600] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
        let p = |x, y, z| Point3::try_new(x * scale, y * scale, z * scale).unwrap();
        let base =
            Curve3::Line(LineSegment::try_new(p(0., 0., 0.), p(3., 0., 0.), tolerance).unwrap());
        let circle = Circle3::try_from_frame(
            p(5., 7., 11.),
            4. * scale,
            Vector3::try_new(1., 0., 0.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Vector3::try_new(0., -1., 0.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            tolerance,
        )
        .unwrap();
        let target = Curve3::Arc(
            CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap(),
        );
        for stretch in [false, true] {
            let morph = CurveFlowPointMorph::try_new(
                base.clone(),
                target.clone(),
                CurveFlowOptions {
                    stretch,
                    ..Default::default()
                },
                tolerance,
            )
            .unwrap();
            for [a, b, c] in [[0., 0.2, 0.4], [1.5, -0.3, 0.5], [3., 0.4, -0.2]] {
                let angle = if stretch {
                    a / 3. * std::f64::consts::FRAC_PI_2
                } else {
                    a / 4.
                };
                let expected = p(
                    5. + (4. - c) * angle.cos(),
                    7. + b,
                    11. + (4. - c) * angle.sin(),
                );
                let actual = morph.morph_point(p(a, b, c)).unwrap();
                assert!(
                    actual.distance_to(expected).unwrap() / scale < 2e-11,
                    "scale=2^{exponent} stretch={stretch} input={:?} actual={:?} expected={:?} lengths={:?}",
                    [a, b, c],
                    actual.to_array().map(|v| v / scale),
                    expected.to_array().map(|v| v / scale),
                    [morph.base_length() / scale, morph.target_length() / scale]
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 10);
    eprintln!("FLOW_BORROWED_PLANE cases={checked}");
}
