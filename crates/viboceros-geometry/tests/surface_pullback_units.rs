use viboceros_geometry::{
    Frame3, NurbsCurve, NurbsCurve2, NurbsSurface, Point2, Point3, Tolerance, Vector3,
};

const SCALES: [i32; 4] = [-20, 0, 40, 80];
const UV: [(i32, i32); 7] = [
    (0, 0),
    (-200, -200),
    (200, 200),
    (-600, -600),
    (600, 600),
    (-600, 600),
    (600, -600),
];

fn setup(exponent: i32) -> (f64, Tolerance, Frame3, NurbsSurface, NurbsCurve) {
    let scale = 2_f64.powi(exponent);
    let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
    let pose = Frame3::try_from_unit_normal(
        Point3::try_new(5. * scale, -7. * scale, 11. * scale).unwrap(),
        Vector3::try_new(2., 3., 6.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
    )
    .unwrap();
    let point = |x, y| pose.point_at([x * scale, y * scale, 0.]).unwrap();
    let surface = NurbsSurface::try_bilinear([
        point(0., 0.),
        point(10., 0.),
        point(8., 10.),
        point(0., 10.),
    ])
    .unwrap();
    // u(t)=t, v(t)=1/5 + 7t/5 - t^2. The surface is
    // S(u,v)=((10-2v)u, 10v, 0), so S(u(t),v(t)) is cubic.
    let curve = NurbsCurve::try_new(
        3,
        vec![
            point(0., 2.),
            point(3.2, 20. / 3.),
            point(82. / 15., 8.),
            point(8.8, 6.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    (scale, tolerance, pose, surface, curve)
}
fn check(
    surface: &NurbsSurface,
    source: &NurbsCurve,
    uv: &NurbsCurve2,
    scale: f64,
    uw: f64,
    vw: f64,
    tolerance: Tolerance,
) -> Result<(f64, f64), String> {
    assert_eq!(uv.domain(), source.domain());
    let bound = surface
        .parameter_curve_deviation_bound(uv, source, tolerance.absolute())
        .map_err(|e| format!("certificate {e:?}"))?
        .ok_or("no continuous certificate")?;
    assert!(bound <= tolerance.absolute());
    let mut model_error = 0_f64;
    let mut uv_error = 0_f64;
    let sampler = source.parameter_sampler().unwrap();
    for i in 0..=64 {
        let t = i as f64 / 64.;
        let p = uv.evaluate(uv.parameter_at(t).unwrap()).unwrap();
        let expected_v = 0.2 + 1.4 * t - t * t;
        uv_error = uv_error
            .max((p.x() / uw - t).abs())
            .max((p.y() / vw - expected_v).abs());
        let actual = surface
            .evaluate(p.x(), p.y())
            .map_err(|e| format!("image {e:?}"))?;
        model_error =
            model_error.max(actual.distance_to(sampler.evaluate(t).unwrap()).unwrap() / scale);
    }
    if uv_error > 2e-8 || model_error > 1e-7 {
        return Err(format!("UV error={uv_error} model error={model_error}"));
    }
    Ok((uv_error, model_error))
}

#[test]
fn regular_pullbacks_preserve_uv_images_source_domains_and_continuous_bounds_across_units() {
    let mut checked = 0;
    let mut failures = Vec::new();
    let mut maximum_uv_error = 0_f64;
    let mut maximum_model_error = 0_f64;
    for exponent in SCALES {
        let (scale, tolerance, _, base, curve) = setup(exponent);
        for (ue, ve) in UV {
            let uw = 2_f64.powi(ue);
            let vw = 2_f64.powi(ve);
            let surface = base.try_reparameterized(0. ..=uw, 0. ..=vw).unwrap();
            let before_surface = surface.clone();
            for te in [-600, 0, 600] {
                let width = 2_f64.powi(te);
                let source = curve.try_reparameterized(0. ..=width).unwrap();
                let before_source = source.clone();
                for mode in 0..3 {
                    checked += 1;
                    let result = match mode {
                        0 => surface.try_pullback_curve(&source, tolerance),
                        1 => surface.try_pullback_curve_certified(&source, tolerance),
                        _ => surface.try_pullback_curve_certified_with_endpoints(
                            &source,
                            [
                                Point2::try_new(0., 0.2 * vw).unwrap(),
                                Point2::try_new(uw, 0.6 * vw).unwrap(),
                            ],
                            tolerance,
                        ),
                    };
                    let label =
                        format!("scale=2^{exponent} UV=2^({ue},{ve}) source=2^{te} mode={mode}");
                    match result {
                        Ok(uv) => match check(&surface, &source, &uv, scale, uw, vw, tolerance) {
                            Ok((u, m)) => {
                                maximum_uv_error = maximum_uv_error.max(u);
                                maximum_model_error = maximum_model_error.max(m);
                            }
                            Err(error) => failures.push(format!("{label}: {error}")),
                        },
                        Err(error) => failures.push(format!("{label}: {error:?}")),
                    }
                    assert_eq!(source, before_source);
                    assert_eq!(surface, before_surface);
                }
            }
        }
    }
    eprintln!(
        "PULLBACK_UNITS cases={checked} failures={} max_uv_error={maximum_uv_error} max_normalized_model_error={maximum_model_error}",
        failures.len()
    );
    assert_eq!(checked, 252);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn translated_uv_origins_do_not_snap_legitimate_near_boundary_points() {
    let mut checked = 0;
    let mut failures = Vec::new();
    for exponent in SCALES {
        let (scale, tolerance, pose, base, _) = setup(exponent);
        let a = 1e-7;
        let b = 1. - 2. * a;
        let coefficients = [9.6 * a, 9.6 * b - 2.8 * a, -2.8 * b + 2. * a, 2. * b];
        let [c0, c1, c2, c3] = coefficients;
        let xs = [
            c0,
            c0 + c1 / 3.,
            c0 + 2. * c1 / 3. + c2 / 3.,
            c0 + c1 + c2 + c3,
        ];
        let ys = [2., 20. / 3., 8., 6.];
        let source = NurbsCurve::try_new(
            3,
            (0..4)
                .map(|i| pose.point_at([scale * xs[i], scale * ys[i], 0.]).unwrap())
                .collect(),
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        for e in [-200, 0, 200] {
            let width = 2_f64.powi(e);
            let u0 = 2_f64.powi(20) * width;
            let v0 = -2_f64.powi(18) * width;
            let surface = base
                .try_reparameterized(u0..=u0 + width, v0..=v0 + width)
                .unwrap();
            for certified in [false, true] {
                checked += 1;
                let label = format!("scale=2^{exponent} chart=2^{e} certified={certified}");
                let result = if certified {
                    surface.try_pullback_curve_certified(&source, tolerance)
                } else {
                    surface.try_pullback_curve(&source, tolerance)
                };
                match result {
                    Ok(uv) => {
                        if !surface
                            .parameter_curve_deviation_bound(&uv, &source, tolerance.absolute())
                            .unwrap()
                            .is_some_and(|b| b <= tolerance.absolute())
                        {
                            failures.push(format!("{label}: uncertified"));
                            continue;
                        }
                        for i in 0..=32 {
                            let t = i as f64 / 32.;
                            let p = uv.evaluate(uv.parameter_at(t).unwrap()).unwrap();
                            let u = (p.x() - u0) / width;
                            let v = (p.y() - v0) / width;
                            let expected_v = 0.2 + 1.4 * t - t * t;
                            if (u - (a + b * t)).abs() > 2e-8 || (v - expected_v).abs() > 2e-8 {
                                failures.push(format!("{label}: wrong UV at {t}"));
                                break;
                            }
                        }
                    }
                    Err(error) => failures.push(format!("{label}: {error:?}")),
                }
            }
        }
    }
    eprintln!(
        "PULLBACK_ORIGINS cases={checked} failures={}",
        failures.len()
    );
    assert_eq!(checked, 24);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn lifted(uv: &NurbsCurve2) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        uv.degree(),
        uv.control_points()
            .iter()
            .map(|p| {
                viboceros_geometry::WeightedPoint3::try_new(
                    Point3::try_new(p.point().x(), p.point().y(), 0.).unwrap(),
                    p.weight(),
                )
                .unwrap()
            })
            .collect(),
        uv.knots().to_vec(),
    )
    .unwrap()
}

#[test]
fn pullback_uses_fractional_one_sided_spans_and_preserves_coarse_native_domains() {
    let mut checked = 0;
    for exponent in [0, 80] {
        let (scale, tolerance, pose, base, smooth) = setup(exponent);
        let kink = NurbsCurve::try_new(
            2,
            [(0., 2.), (2.4, 3.5), (4.5, 5.), (6.7, 5.5), (8.8, 6.)]
                .map(|(x, y)| pose.point_at([scale * x, scale * y, 0.]).unwrap())
                .to_vec(),
            vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
        )
        .unwrap();
        for (ue, ve) in [(-600, 600), (600, -600)] {
            let uw = 2_f64.powi(ue);
            let vw = 2_f64.powi(ve);
            let surface = base.try_reparameterized(0. ..=uw, 0. ..=vw).unwrap();
            for te in [-600, 0, 600] {
                for coarse in [false, true] {
                    for kind in [0, 1] {
                        if coarse && kind == 1 {
                            continue;
                        }
                        let width = 2_f64.powi(te);
                        let domain = if coarse {
                            width..=width.next_up()
                        } else {
                            0. ..=width
                        };
                        let source = if kind == 0 { &smooth } else { &kink }
                            .try_reparameterized(domain.clone())
                            .unwrap();
                        let before = source.clone();
                        let (uv, bound) = surface
                            .try_pullback_curve_certified_with_bound(&source, None, tolerance)
                            .unwrap();
                        assert_eq!(uv.domain(), domain);
                        assert!(bound <= tolerance.absolute());
                        let lifted = lifted(&uv);
                        let uv_samples = lifted.parameter_sampler().unwrap();
                        let source_samples = source.parameter_sampler().unwrap();
                        for i in 0..=64 {
                            let t = i as f64 / 64.;
                            let p = uv_samples.evaluate(t).unwrap();
                            let v = if kind == 0 {
                                0.2 + 1.4 * t - t * t
                            } else if t <= 0.5 {
                                0.2 + 0.6 * t
                            } else {
                                0.4 + 0.2 * t
                            };
                            assert!(
                                (p.x() / uw - t).abs() < 2e-8 && (p.y() / vw - v).abs() < 2e-8,
                                "coarse={coarse} kind={kind} t={t}"
                            );
                            let image = surface.evaluate(p.x(), p.y()).unwrap();
                            assert!(
                                image
                                    .distance_to(source_samples.evaluate(t).unwrap())
                                    .unwrap()
                                    / scale
                                    < 1e-7
                            );
                        }
                        assert_eq!(source, before);
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 36);
    eprintln!("PULLBACK_SPANS cases={checked}");
}

#[test]
fn certified_pullbacks_keep_absolute_error_and_fixed_endpoint_admission() {
    let mut checked = 0;
    for exponent in SCALES {
        let (scale, tolerance, pose, base, source) = setup(exponent);
        for (ue, ve) in UV {
            let uw = 2_f64.powi(ue);
            let vw = 2_f64.powi(ve);
            let surface = base.try_reparameterized(0. ..=uw, 0. ..=vw).unwrap();
            let before = (surface.clone(), source.clone());
            for (factor, accepted) in [(0.5, true), (8., false)] {
                let shift = pose
                    .z_axis()
                    .as_vector()
                    .scaled(factor * tolerance.absolute())
                    .unwrap();
                let displaced = NurbsCurve::try_new_rational(
                    source.degree(),
                    source
                        .control_points()
                        .iter()
                        .map(|p| {
                            viboceros_geometry::WeightedPoint3::try_new(
                                p.point().translated(shift).unwrap(),
                                p.weight(),
                            )
                            .unwrap()
                        })
                        .collect(),
                    source.knots().to_vec(),
                )
                .unwrap();
                let result =
                    surface.try_pullback_curve_certified_with_bound(&displaced, None, tolerance);
                assert_eq!(
                    result.is_ok(),
                    accepted,
                    "scale={scale} UV=({ue},{ve}) offset={factor} tolerance"
                );
                if let Ok((_, bound)) = result {
                    assert!(bound <= tolerance.absolute());
                }
                checked += 1;
            }
            assert!(
                surface
                    .try_pullback_curve_certified_with_endpoints(
                        &source,
                        [
                            Point2::try_new(0., 0.2 * vw).unwrap(),
                            Point2::try_new(uw, 0.61 * vw).unwrap()
                        ],
                        tolerance
                    )
                    .is_err()
            );
            checked += 1;
            assert_eq!((surface, source.clone()), before);
        }
    }
    assert_eq!(checked, 84);
    eprintln!("PULLBACK_ADMISSION cases={checked}");
}
