use super::*;

fn profile(scale: Real, weight: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [[1., 0., 1.], [1., 1., 1.], [0., 1., 1.]]
            .into_iter()
            .zip([1., std::f64::consts::FRAC_1_SQRT_2, 1.])
            .map(|(p, w)| {
                WeightedPoint3::try_new(Point3::try_from(p.map(|x| x * scale)).unwrap(), w * weight)
                    .unwrap()
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn poles_preserve_exact_points_under_weight_gauges_and_model_units() {
    let ends = LoftEndPoints {
        start: Some(Point3::try_new(0., 0., -1.).unwrap()),
        end: Some(Point3::try_new(0., 0., 3.).unwrap()),
    };
    let expected =
        try_loft_nurbs_curves_with_end_points(&[profile(1., 1.)], LoftStyle::Normal, ends).unwrap();
    for scale in [1., 2_f64.powi(350), 2_f64.powi(-350)] {
        for weight in [1., 2_f64.powi(600), -2_f64.powi(600), 2_f64.powi(-600)] {
            let source = [profile(scale, weight)];
            let before = source.clone();
            let scaled = |p: Option<Point3>| {
                p.map(|p| Point3::try_from(p.to_array().map(|x| x * scale)).unwrap())
            };
            let actual = try_loft_nurbs_curves_with_end_points(
                &source,
                LoftStyle::Normal,
                LoftEndPoints {
                    start: scaled(ends.start),
                    end: scaled(ends.end),
                },
            )
            .unwrap();
            assert_eq!(source, before);
            for u in 0..=8 {
                for v in 0..=8 {
                    let a = actual
                        .evaluate(
                            actual.parameter_at_u(u as f64 / 8.).unwrap(),
                            actual.parameter_at_v(v as f64 / 8.).unwrap(),
                        )
                        .unwrap();
                    let b = expected
                        .evaluate(
                            expected.parameter_at_u(u as f64 / 8.).unwrap(),
                            expected.parameter_at_v(v as f64 / 8.).unwrap(),
                        )
                        .unwrap();
                    for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
                        assert!((a / scale - b).abs() < 1e-12);
                    }
                }
            }
        }
    }
}

#[test]
fn point_end_limits_and_unsupported_style_precede_backend_work() {
    let curve = profile(1., 1.);
    let point = Point3::try_new(0., 0., 0.).unwrap();
    for (curves, style, ends) in [
        (
            vec![],
            LoftStyle::Normal,
            LoftEndPoints {
                start: Some(point),
                end: Some(point),
            },
        ),
        (
            vec![curve.clone()],
            LoftStyle::Normal,
            LoftEndPoints::default(),
        ),
        (
            vec![curve.clone()],
            LoftStyle::Uniform,
            LoftEndPoints {
                start: Some(point),
                end: None,
            },
        ),
        (
            vec![curve; MAX_LOFT_SECTIONS - 1],
            LoftStyle::Normal,
            LoftEndPoints {
                start: Some(point),
                end: Some(point),
            },
        ),
    ] {
        let mut called = false;
        let result: Result<_, GeometryError> = try_loft_nurbs_curves_with_end_points_and_operations(
            &curves,
            style,
            ends,
            &mut |_| {
                called = true;
                Ok(Vec::new())
            },
            &mut |_| unreachable!("invalid point request must not reach solving"),
        );
        assert!(result.is_err());
        assert!(!called);
    }
}

#[test]
fn bad_backend_layout_is_rejected_before_pole_insertion() {
    let result: Result<_, GeometryError> = try_loft_nurbs_curves_with_end_points_and_operations(
        &[profile(1., 1.)],
        LoftStyle::Normal,
        LoftEndPoints {
            start: Some(Point3::try_new(0., 0., 0.).unwrap()),
            end: None,
        },
        &mut |_| Ok(Vec::new()),
        &mut |_| unreachable!(),
    );
    assert!(matches!(
        result,
        Err(GeometryError::InvalidControlNet { .. })
    ));
}

#[test]
fn linear_profiles_honor_poles_and_form_valid_sheets_for_every_supported_style() {
    use crate::{Brep, Tolerance};
    let point = |p| Point3::try_from(p).unwrap();
    let start = point([-0.5, 0.25, -1.]);
    for count in 1..=4 {
        let curves = (0..count)
            .map(|i| {
                let i = i as f64;
                NurbsCurve::try_new(
                    1,
                    vec![
                        point([0.2 * i, 0., 1. + 2. * i]),
                        point([2. + 0.7 * i, 0., 1. + 2. * i]),
                    ],
                    vec![0., 0., 1., 1.],
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        for style in [
            LoftStyle::Normal,
            LoftStyle::Loose,
            LoftStyle::Tight,
            LoftStyle::Straight,
        ] {
            let end = point([0.5, -0.25, 3. + 2. * count as f64]);
            for ends in [
                LoftEndPoints {
                    start: Some(start),
                    end: None,
                },
                LoftEndPoints {
                    start: None,
                    end: Some(end),
                },
                LoftEndPoints {
                    start: Some(start),
                    end: Some(end),
                },
            ] {
                let surface = try_loft_nurbs_curves_with_end_points(&curves, style, ends).unwrap();
                let brep = Brep::try_surface_grid(&surface, &[], &[], Tolerance::DEFAULT).unwrap();
                assert!(!brep.is_solid());
                let mesh = brep
                    .polygon_mesh(0., false, false, Tolerance::DEFAULT)
                    .unwrap();
                assert!(mesh.topology().is_manifold());
                for p in [ends.start, ends.end].into_iter().flatten() {
                    assert!(brep.vertices().iter().any(|v| v.point() == p));
                }
            }
        }
    }
}
