#![cfg(feature = "native")]
use viboceros_geometry::{CurveFitStation, NurbsCurve, Point3, Tolerance};
use viboceros_smlib::{
    GordonCurveFitClock, GordonCurveFitInput, fit_gordon_curve_family,
    fit_gordon_curve_family_with_clock,
};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn pins(source: &NurbsCurve, middle: f64) -> Vec<CurveFitStation> {
    [0., 0.5, 1.]
        .into_iter()
        .zip([0., middle, 1.])
        .map(|(s, t)| {
            let source_parameter = source.parameter_at(s).unwrap();
            CurveFitStation {
                source_parameter,
                fit_parameter: t,
                point: source.evaluate(source_parameter).unwrap(),
            }
        })
        .collect()
}
#[test]
fn family_fits_lines_in_shared_c2_basis_and_preserves_independent_domains() {
    verify_line_family(None);
}
#[test]
fn local_clock_lines_keep_fixed_contacts_and_shared_nonbinary_knots() {
    for transition_fraction in [0.1, 0.25, 0.5] {
        verify_line_family(Some(GordonCurveFitClock::Local {
            transition_fraction,
        }));
    }
}
fn verify_line_family(policy: Option<GordonCurveFitClock>) {
    let sources = [
        NurbsCurve::try_new(
            1,
            vec![p(0., 0., 0.), p(10., 0., 0.)],
            vec![-3., -3., 7., 7.],
        )
        .unwrap(),
        NurbsCurve::try_new(
            1,
            vec![p(0., 2., 1.), p(10., 2., 1.)],
            vec![100., 100., 200., 200.],
        )
        .unwrap(),
    ];
    let before = sources.clone();
    let stations = sources.each_ref().map(|s| pins(s, 0.3));
    let requests = sources
        .iter()
        .zip(&stations)
        .map(|(curve, stations)| GordonCurveFitInput { curve, stations })
        .collect::<Vec<_>>();
    let result = match policy {
        None => fit_gordon_curve_family(&requests, &[4, 4], 1e-7, Tolerance::DEFAULT),
        Some(policy) => {
            fit_gordon_curve_family_with_clock(&requests, &[4, 4], 1e-7, Tolerance::DEFAULT, policy)
        }
    }
    .unwrap();
    assert_eq!(result.curves.len(), 2);
    assert!(result.maximum_sampled_point_distance <= 1e-7);
    assert_eq!(result.curves[0].knots(), result.curves[1].knots());
    for (i, curve) in result.curves.iter().enumerate() {
        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.control_points().len(), 11);
        assert!(curve.control_points().iter().all(|p| p.weight() == 1.));
        for t in [0., 0.1, 0.3, 0.6, 1.] {
            let point = curve.evaluate(t).unwrap();
            assert!((point.y() - 2. * i as f64).abs() < 1e-9);
            assert!((point.z() - i as f64).abs() < 1e-9);
        }
        assert!(
            curve
                .evaluate(0.3)
                .unwrap()
                .distance_to(stations[i][1].point)
                .unwrap()
                < 1e-9
        );
    }
    assert_eq!(sources, before);
}
#[test]
fn family_fit_rejects_invalid_layouts_before_native_fitting() {
    let curve =
        NurbsCurve::try_new(1, vec![p(0., 0., 0.), p(10., 0., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let stations = pins(&curve, 0.5);
    let request = [GordonCurveFitInput {
        curve: &curve,
        stations: &stations,
    }];
    for spans in [vec![], vec![3, 4], vec![64, 64], vec![4, 4, 4]] {
        assert!(fit_gordon_curve_family(&request, &spans, 1e-6, Tolerance::DEFAULT).is_err());
    }
    for tolerance in [0., -1., f64::NAN] {
        assert!(fit_gordon_curve_family(&request, &[4, 4], tolerance, Tolerance::DEFAULT).is_err());
    }
    for transition_fraction in [
        0.,
        -1.,
        0.5_f64.next_up(),
        f64::NAN,
        f64::INFINITY,
        f64::from_bits(1),
    ] {
        assert!(
            fit_gordon_curve_family_with_clock(
                &request,
                &[4, 4],
                1e-7,
                Tolerance::DEFAULT,
                GordonCurveFitClock::Local {
                    transition_fraction
                }
            )
            .is_err()
        );
    }
    let mut bad = stations.clone();
    bad[1].point = p(50., 1., 0.);
    assert!(
        fit_gordon_curve_family(
            &[GordonCurveFitInput {
                curve: &curve,
                stations: &bad
            }],
            &[4, 4],
            1e-6,
            Tolerance::DEFAULT
        )
        .is_err()
    );
    bad = stations.clone();
    bad[1].fit_parameter = 1.;
    assert!(
        fit_gordon_curve_family(
            &[GordonCurveFitInput {
                curve: &curve,
                stations: &bad
            }],
            &[4, 4],
            1e-6,
            Tolerance::DEFAULT
        )
        .is_err()
    );
    bad = stations.clone();
    bad[1].source_parameter = f64::NAN;
    assert!(
        fit_gordon_curve_family(
            &[GordonCurveFitInput {
                curve: &curve,
                stations: &bad
            }],
            &[4, 4],
            1e-6,
            Tolerance::DEFAULT
        )
        .is_err()
    );
}

fn value(u: f64, v: f64) -> Point3 {
    p(
        4. * u,
        3. * v,
        2. * u * v + 4. * u * (1. - u) + 2. * v * (1. - v) + 8. * u * (1. - u) * v * (1. - v),
    )
}
fn network() -> [Vec<NurbsCurve>; 2] {
    [true, false].map(|along_u| {
        [0., 0.5, 1.]
            .into_iter()
            .map(|station| {
                let f = |t| {
                    if along_u {
                        value(t, station)
                    } else {
                        value(station, t)
                    }
                };
                let a = f(0.).to_array();
                let b = f(0.5).to_array();
                let c = f(1.).to_array();
                let middle =
                    Point3::try_from(std::array::from_fn(|k| 2. * b[k] - (a[k] + c[k]) / 2.))
                        .unwrap();
                NurbsCurve::try_new(2, vec![f(0.), middle, f(1.)], vec![0., 0., 0., 1., 1., 1.])
                    .unwrap()
            })
            .collect()
    })
}
#[test]
fn geometric_family_fits_feed_a_c2_surface_with_complete_original_images() {
    verify_complete_original_images(GordonCurveFitClock::WholeInterval);
}
#[test]
fn local_clock_family_fits_feed_a_c2_surface_with_complete_original_images() {
    verify_complete_original_images(GordonCurveFitClock::Local {
        transition_fraction: 0.5,
    });
}
fn verify_complete_original_images(policy: GordonCurveFitClock) {
    let original = network();
    let before = original.clone();
    let mut fitted = vec![];
    for family in &original {
        let stations = family.iter().map(|c| pins(c, 0.5)).collect::<Vec<_>>();
        let requests = family
            .iter()
            .zip(&stations)
            .map(|(curve, stations)| GordonCurveFitInput { curve, stations })
            .collect::<Vec<_>>();
        let result = fit_gordon_curve_family_with_clock(
            &requests,
            &[60, 60],
            1e-7,
            Tolerance::DEFAULT,
            policy,
        )
        .unwrap();
        assert!(
            result
                .curves
                .iter()
                .all(|c| c.knots() == result.curves[0].knots())
        );
        fitted.push(result.curves);
    }
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let surface = viboceros_smlib::prepared_gordon_surface(
        &fitted[0],
        &fitted[1],
        &[0., 0.5, 1.],
        &[0., 0.5, 1.],
        tolerance,
        tolerance,
    )
    .unwrap();
    for (family, along_u) in original.iter().zip([true, false]) {
        for (index, source) in family.iter().enumerate() {
            let fixed = index as f64 / 2.;
            let endpoints = if along_u {
                [[0., fixed], [1., fixed]]
            } else {
                [[fixed, 0.], [fixed, 1.]]
            };
            let endpoints =
                endpoints.map(|p| viboceros_geometry::Point2::try_new(p[0], p[1]).unwrap());
            let (uv, bound) = surface
                .try_pullback_curve_certified_with_bound(source, Some(endpoints), tolerance)
                .unwrap();
            assert_eq!(uv.domain(), source.domain());
            assert!(bound <= tolerance.absolute());
        }
    }
    assert!(
        surface
            .knots_u()
            .chunk_by(|a, b| a == b)
            .filter(|k| k[0] > 0. && k[0] < 1.)
            .all(|k| k.len() == 1)
    );
    assert_eq!(original, before);
}

#[test]
fn family_fit_rejects_a_geometrically_inadequate_shared_basis() {
    let original = network();
    let source = &original[0][1];
    let stations = pins(source, 0.3);
    let before = source.clone();
    let result = fit_gordon_curve_family(
        &[GordonCurveFitInput {
            curve: source,
            stations: &stations,
        }],
        &[4, 4],
        1e-12,
        Tolerance::DEFAULT,
    );
    // Either an insufficient measured fit or an inconclusive bounded native
    // solve must decline this valid layout, without returning unchecked output.
    assert!(result.is_err());
    assert_eq!(source, &before);
}

#[test]
fn family_fit_handles_a_remote_negative_weight_arc_without_mutating_its_domain() {
    use viboceros_geometry::WeightedPoint3;
    let source = NurbsCurve::try_new_rational(
        2,
        [
            (p(100_001., -30_000., 7.), -1.),
            (p(100_001., -29_999., 7.), -std::f64::consts::FRAC_1_SQRT_2),
            (p(100_000., -29_999., 7.), -1.),
        ]
        .into_iter()
        .map(|(point, weight)| WeightedPoint3::try_new(point, weight).unwrap())
        .collect(),
        vec![-3., -3., -3., 7., 7., 7.],
    )
    .unwrap();
    let before = source.clone();
    let stations = pins(&source, 0.5);
    let numerical = Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap();
    let result = fit_gordon_curve_family(
        &[GordonCurveFitInput {
            curve: &source,
            stations: &stations,
        }],
        &[60, 60],
        1e-7,
        numerical,
    )
    .unwrap();
    let fitted = &result.curves[0];
    assert_eq!(fitted.domain(), 0. ..=1.);
    assert!(result.maximum_sampled_point_distance <= 1e-7);
    for i in 0..=128 {
        let point = fitted.evaluate(i as f64 / 128.).unwrap();
        assert!(((point.x() - 100_000.).hypot(point.y() + 30_000.) - 1.).abs() < 5e-7);
        assert!((point.z() - 7.).abs() < 1e-8);
    }
    assert_eq!(source, before);
}
