#![cfg(feature = "native")]
use viboceros_geometry::{
    Circle3, Curve3, CurveOffsetCornerStyle, GeometryError, NurbsCurve, Point3, Tolerance,
    UnitVector3, WeightedPoint3,
};

fn point(x: f64, y: f64, plane: usize) -> Point3 {
    Point3::try_from(match plane {
        1 => [0., x, y],
        2 => [y, 0., x],
        _ => [x, y, 0.],
    })
    .unwrap()
}

#[test]
fn native_rational_circle_offsets_preserve_seams_side_and_scaled_world_frames() {
    let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-10).unwrap();
    for scale in [0.01, 1., 100.] {
        for plane in 0..3 {
            let n = normal(plane, tolerance);
            let center = point(13., -7., plane);
            let source = Circle3::try_new(center, 2. * scale, n, tolerance)
                .unwrap()
                .to_nurbs()
                .unwrap()
                .try_reparameterized(-4. ..=7.)
                .unwrap();
            let before = source.clone();
            for distance in [-0.125 * scale, 0.125 * scale] {
                let result = viboceros_smlib::offset_curve_parts(
                    &Curve3::NurbsCurve(source.clone()),
                    distance,
                    n,
                    tolerance,
                    CurveOffsetCornerStyle::None,
                )
                .unwrap();
                assert_eq!(result.len(), 1);
                let Curve3::NurbsCurve(result) = &result[0] else {
                    panic!("expected fitted NURBS")
                };
                assert_eq!(result.domain(), source.domain());
                assert!(result.is_closed().unwrap());
                assert_eq!(result.evaluate(-4.).unwrap(), result.evaluate(7.).unwrap());
                for i in 0..=1024 {
                    let parameter = source.parameter_at(i as f64 / 1024.).unwrap();
                    let original = source.evaluate(parameter).unwrap();
                    let expected = center
                        .translated(
                            center
                                .vector_to(original)
                                .unwrap()
                                .scaled(1. - distance / (2. * scale))
                                .unwrap(),
                        )
                        .unwrap();
                    assert!(
                        result
                            .evaluate(parameter)
                            .unwrap()
                            .distance_to(expected)
                            .unwrap()
                            <= tolerance.absolute()
                    );
                }
            }
            assert!(matches!(
                viboceros_smlib::offset_curve_parts(
                    &Curve3::NurbsCurve(source.clone()),
                    3. * scale,
                    n,
                    tolerance,
                    CurveOffsetCornerStyle::None
                ),
                Err(GeometryError::Degenerate {
                    context: "NURBS offset cusp"
                })
            ));
            assert_eq!(source, before);
        }
    }
}
fn normal(plane: usize, tolerance: Tolerance) -> UnitVector3 {
    let xyz = match plane {
        1 => [1., 0., 0.],
        2 => [0., 1., 0.],
        _ => [0., 0., 1.],
    };
    UnitVector3::try_new(xyz[0], xyz[1], xyz[2], tolerance).unwrap()
}

#[test]
fn native_parabola_offsets_match_analytic_points_across_charts_planes_and_gauges() {
    for absolute in [1e-6, 1e-9] {
        let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-10).unwrap();
        for (lo, hi) in [(0., 1.), (-4., 7.)] {
            for gauge in [1., 1e-280, -1e280] {
                for plane in 0..3 {
                    let controls = [(0., 0.), (1., 2.), (2., 0.)]
                        .into_iter()
                        .map(|(x, y)| WeightedPoint3::try_new(point(x, y, plane), gauge).unwrap())
                        .collect();
                    let source =
                        NurbsCurve::try_new_rational(2, controls, vec![lo, lo, lo, hi, hi, hi])
                            .unwrap();
                    let before = source.clone();
                    for distance in [-0.125, 0.125] {
                        let parts = viboceros_smlib::offset_curve_parts(
                            &Curve3::NurbsCurve(source.clone()),
                            distance,
                            normal(plane, tolerance),
                            tolerance,
                            CurveOffsetCornerStyle::None,
                        )
                        .unwrap();
                        assert_eq!(parts.len(), 1);
                        let Curve3::NurbsCurve(result) = &parts[0] else {
                            panic!("expected fitted NURBS")
                        };
                        assert_eq!(result.domain(), source.domain());
                        for i in 0..=1024 {
                            let u = i as f64 / 1024.;
                            let dy = 4. - 8. * u;
                            let speed = 2_f64.hypot(dy);
                            let expected = point(
                                2. * u - distance * dy / speed,
                                4. * u * (1. - u) + distance * 2. / speed,
                                plane,
                            );
                            let error = result
                                .evaluate(result.parameter_at(u).unwrap())
                                .unwrap()
                                .distance_to(expected)
                                .unwrap();
                            assert!(
                                error <= absolute,
                                "{absolute}, {lo}..{hi}, {gauge}, {plane}, {distance}, {u}: {error}"
                            );
                        }
                    }
                    assert_eq!(source, before);
                }
            }
        }
    }
}

#[test]
fn native_fitter_declines_unsupported_bases_and_subthreshold_offsets() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let n = normal(0, tolerance);
    let operations = viboceros_smlib::curve_offset_operations();
    let degree18 = NurbsCurve::try_new(
        18,
        (0..=18).map(|i| point(i as f64 / 18., 0., 0)).collect(),
        [vec![0.; 19], vec![1.; 19]].concat(),
    )
    .unwrap();
    let mixed = NurbsCurve::try_new_rational(
        2,
        [1., -0.1, 1.]
            .into_iter()
            .enumerate()
            .map(|(i, w)| WeightedPoint3::try_new(point(i as f64, 0., 0), w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let broken = NurbsCurve::try_new(
        2,
        (0..6).map(|i| point(i as f64, 0., 0)).collect(),
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    for source in [&degree18, &mixed, &broken] {
        assert!(
            (operations.smooth)(source, 0.125, n, tolerance, false)
                .unwrap()
                .is_none()
        );
    }
    let source = NurbsCurve::try_new(
        2,
        vec![point(0., 0., 0), point(1., 0., 0), point(2., 0., 0)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert!(
        (operations.smooth)(&source, 1e-13, n, tolerance, false)
            .unwrap()
            .is_none()
    );
    let tiny_tolerance = Tolerance::try_new(1e-15, 1e-15, 1e-10).unwrap();
    for (source, distance, tolerance) in [
        (degree18, 0.125, tolerance),
        (source, 1e-13, tiny_tolerance),
    ] {
        let result = viboceros_smlib::offset_curve_parts(
            &Curve3::NurbsCurve(source),
            distance,
            n,
            tolerance,
            CurveOffsetCornerStyle::None,
        )
        .unwrap();
        for i in 0..=32 {
            let u = i as f64 / 32.;
            let p = result[0]
                .as_ref()
                .evaluate(result[0].as_ref().parameter_at(u).unwrap())
                .unwrap();
            assert!((p.y() - distance).abs() <= tolerance.absolute());
        }
    }
}
