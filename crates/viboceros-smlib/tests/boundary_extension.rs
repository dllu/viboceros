#![cfg(feature = "native")]
use viboceros_geometry::{
    Brep, CurveExtensionBoundary as B, CurveExtensionSide as S, CurveExtensionStyle as C,
    GeometryError, NurbsCurve, NurbsSurface, Point3, Tolerance, WeightedPoint3,
};

fn point(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}

fn boundary(x: f64, kind: usize) -> B {
    match kind {
        0 => B::Curve(
            NurbsCurve::try_new(
                1,
                vec![point(x, -100.), point(x, 100.)],
                vec![0., 0., 200., 200.],
            )
            .unwrap(),
        ),
        1 => B::Surface(
            NurbsSurface::try_bilinear([
                Point3::try_new(x, -100., -1.).unwrap(),
                Point3::try_new(x, 100., -1.).unwrap(),
                Point3::try_new(x, 100., 1.).unwrap(),
                Point3::try_new(x, -100., 1.).unwrap(),
            ])
            .unwrap(),
        ),
        _ => {
            let ring = NurbsCurve::try_new(
                1,
                vec![
                    Point3::try_new(x, -100., -1.).unwrap(),
                    Point3::try_new(x, 100., -1.).unwrap(),
                    Point3::try_new(x, 100., 1.).unwrap(),
                    Point3::try_new(x, -100., 1.).unwrap(),
                    Point3::try_new(x, -100., -1.).unwrap(),
                ],
                vec![0., 0., 200., 202., 402., 404., 404.],
            )
            .unwrap();
            B::Brep(Brep::try_planar_face(&ring, Tolerance::DEFAULT).unwrap())
        }
    }
}

fn outputs(
    source: &NurbsCurve,
    side: S,
    style: C,
    bounds: &[B],
    join: usize,
    native: bool,
) -> Vec<NurbsCurve> {
    if native {
        match join {
            0 => vec![
                viboceros_smlib::extend_curve_to_boundaries(
                    source,
                    side,
                    style,
                    bounds,
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ],
            1 => vec![
                viboceros_smlib::join_curve_to_boundaries(
                    source,
                    side,
                    style,
                    bounds,
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ],
            _ => viboceros_smlib::separate_curve_extensions_to_boundaries(
                source,
                side,
                style,
                bounds,
                Tolerance::DEFAULT,
            )
            .unwrap(),
        }
    } else {
        match join {
            0 => vec![
                source
                    .try_merged_to_boundaries(side, style, bounds, Tolerance::DEFAULT)
                    .unwrap(),
            ],
            1 => vec![
                source
                    .try_joined_to_boundaries(side, style, bounds, Tolerance::DEFAULT)
                    .unwrap(),
            ],
            _ => source
                .try_separate_extensions_to_boundaries(side, style, bounds, Tolerance::DEFAULT)
                .unwrap(),
        }
    }
}

fn same_definitions(actual: &[NurbsCurve], expected: &[NurbsCurve]) {
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert_eq!(a.degree(), b.degree());
        assert_eq!(a.control_points().len(), b.control_points().len());
        assert_eq!(a.knots().len(), b.knots().len());
        for (x, y) in a.knots().iter().zip(b.knots()) {
            assert!(
                (x - y).abs() < 1e-9,
                "knots differ: {x} vs {y}; actual={a:?}; expected={b:?}"
            );
        }
        for (x, y) in a.control_points().iter().zip(b.control_points()) {
            assert!(x.point().distance_to(y.point()).unwrap() < 1e-8);
            assert!((x.weight() / y.weight() - 1.).abs() < 1e-12);
        }
        for i in 0..=32 {
            let p = a.evaluate(a.parameter_at(i as f64 / 32.).unwrap()).unwrap();
            let q = b.evaluate(b.parameter_at(i as f64 / 32.).unwrap()).unwrap();
            assert!(p.distance_to(q).unwrap() < 1e-8);
        }
    }
}

#[test]
fn native_boundary_extension_preserves_nearest_hits_all_join_modes_and_unclamped_sources() {
    // Linear/quadratic precision in the B-spline basis gives independent
    // controls for C(t)=(4t,6t(1-t),0), including the unclamped definition.
    for unclamped in [false, true] {
        let knots = if unclamped {
            vec![-2., -1., 0., 0.5, 1., 2., 3.]
        } else {
            vec![0., 0., 0., 1., 1., 1.]
        };
        for gauge in [1., 1e-280, -1e280] {
            let controls = (0..knots.len() - 3)
                .map(|i| {
                    let a = knots[i + 1];
                    let b = knots[i + 2];
                    let g = (a + b) * 0.5;
                    WeightedPoint3::try_new(point(4. * g, 6. * g - 6. * a * b), gauge).unwrap()
                })
                .collect();
            let source = NurbsCurve::try_new_rational(2, controls, knots.clone()).unwrap();
            let before = source.clone();
            for kind in 0..3 {
                // Put the farther candidates first to exercise nearest-hit ordering.
                let bounds = vec![
                    boundary(12., kind),
                    boundary(-3., kind),
                    boundary(7., kind),
                    boundary(-2., kind),
                ];
                let bounds_before = bounds.clone();
                for side in [S::Start, S::End, S::Both] {
                    for style in [C::Smooth, C::Natural] {
                        for join in 0..3 {
                            let actual = outputs(&source, side, style, &bounds, join, true);
                            let expected = outputs(&source, side, style, &bounds, join, false);
                            same_definitions(&actual, &expected);
                            let start = if side == S::End { 0. } else { -0.5 };
                            let end = if side == S::Start { 1. } else { 1.75 };
                            assert!(
                                (*actual.first().unwrap().domain().start()
                                    - if join == 2 && side == S::End {
                                        1.
                                    } else {
                                        start
                                    })
                                .abs()
                                    < 1e-9
                            );
                            assert!(
                                (*actual.last().unwrap().domain().end()
                                    - if join == 2 && side == S::Start {
                                        0.
                                    } else {
                                        end
                                    })
                                .abs()
                                    < 1e-9
                            );
                            for curve in actual {
                                for i in 0..=32 {
                                    let t = curve.parameter_at(i as f64 / 32.).unwrap();
                                    assert!(
                                        curve
                                            .evaluate(t)
                                            .unwrap()
                                            .distance_to(point(4. * t, 6. * t * (1. - t)))
                                            .unwrap()
                                            < 1e-9
                                    );
                                }
                            }
                        }
                    }
                }
                assert_eq!(bounds, bounds_before);
            }
            assert_eq!(source, before);
        }
    }
}

#[test]
fn rational_boundary_extension_matches_independent_quotient_and_restores_signed_gauges() {
    for gauge in [1., 1e-280, -1e280] {
        let source = NurbsCurve::try_new_rational(
            2,
            [
                (point(0., 0.), 1.25),
                (point(2., 3.), 1.),
                (point(2., 0.), 1.25),
            ]
            .into_iter()
            .map(|(p, w)| WeightedPoint3::try_new(p, w * gauge).unwrap())
            .collect(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let before = source.clone();
        let bounds = vec![B::Curve(
            NurbsCurve::try_new(
                1,
                vec![point(-20., -1.), point(20., -1.)],
                vec![0., 0., 40., 40.],
            )
            .unwrap(),
        )];
        for join in 0..3 {
            let actual = outputs(&source, S::Both, C::Smooth, &bounds, join, true);
            same_definitions(
                &actual,
                &outputs(&source, S::Both, C::Smooth, &bounds, join, false),
            );
            let delta = (21_f64 / 11.).sqrt();
            assert!((*actual.first().unwrap().domain().start() - (1. - delta) * 0.5).abs() < 1e-9);
            assert!((*actual.last().unwrap().domain().end() - (1. + delta) * 0.5).abs() < 1e-9);
            for curve in actual {
                for i in 0..=32 {
                    let t = curve.parameter_at(i as f64 / 32.).unwrap();
                    let w = 1.25 - 0.5 * t + 0.5 * t * t;
                    let expected = point((4. * t - 1.5 * t * t) / w, 6. * t * (1. - t) / w);
                    assert!(curve.evaluate(t).unwrap().distance_to(expected).unwrap() < 1e-9);
                }
            }
        }
        assert_eq!(source, before);
    }
}

#[test]
fn boundary_extension_preserves_full_order_breaks_and_degree_eighteen_fallback() {
    let full = NurbsCurve::try_new(
        2,
        (0..6).map(|i| point(i as f64, (i % 3) as f64)).collect(),
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    let high = NurbsCurve::try_new(
        18,
        (0..=18).map(|i| point(i as f64, (i * i) as f64)).collect(),
        std::iter::repeat_n(0., 19)
            .chain(std::iter::repeat_n(1., 19))
            .collect(),
    )
    .unwrap();
    for source in [full, high] {
        let end = if source.degree() == 18 { 21.6 } else { 7. };
        let bounds = vec![B::Curve(
            NurbsCurve::try_new(
                1,
                vec![point(end, -10_000.), point(end, 10_000.)],
                vec![0., 0., 20_000., 20_000.],
            )
            .unwrap(),
        )];
        for join in 0..3 {
            same_definitions(
                &outputs(&source, S::End, C::Smooth, &bounds, join, true),
                &outputs(&source, S::End, C::Smooth, &bounds, join, false),
            );
        }
    }
}

#[test]
fn native_missing_and_closed_boundary_requests_keep_geometry_errors_and_sources() {
    let source = NurbsCurve::try_new(
        2,
        vec![point(0., 0.), point(2., 3.), point(4., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    assert!(matches!(
        viboceros_smlib::extend_curve_to_boundaries(
            &source,
            S::End,
            C::Smooth,
            &[],
            Tolerance::DEFAULT
        ),
        Err(viboceros_smlib::Error::Geometry(
            GeometryError::EmptyCurveExtensionBoundaries
        ))
    ));
    let missing = viboceros_smlib::extend_curve_to_boundaries(
        &source,
        S::End,
        C::Smooth,
        &[boundary(-2., 0)],
        Tolerance::DEFAULT,
    );
    assert!(
        matches!(
            missing,
            Err(viboceros_smlib::Error::Geometry(
                GeometryError::CurveExtensionBoundaryNotFound
            ))
        ),
        "unexpected missing-hit result: {missing:?}"
    );
    assert_eq!(source, before);
    let closed = NurbsCurve::try_new(
        1,
        vec![
            point(0., 0.),
            point(2., 0.),
            point(2., 2.),
            point(0., 2.),
            point(0., 0.),
        ],
        vec![0., 0., 2., 4., 6., 8., 8.],
    )
    .unwrap();
    assert!(closed.is_closed().unwrap());
    assert!(matches!(
        viboceros_smlib::join_curve_to_boundaries(
            &closed,
            S::End,
            C::Smooth,
            &[boundary(7., 0)],
            Tolerance::DEFAULT
        ),
        Err(viboceros_smlib::Error::Geometry(
            GeometryError::CurveExtensionMustBeOpen
        ))
    ));
}

#[test]
fn both_sides_accepts_a_real_hit_when_the_other_side_has_no_boundary() {
    let source = NurbsCurve::try_new(
        2,
        vec![point(0., 0.), point(2., 3.), point(4., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    for kind in 0..3 {
        let bounds = vec![boundary(7., kind)];
        for join in 0..3 {
            let actual = outputs(&source, S::Both, C::Smooth, &bounds, join, true);
            let expected = outputs(&source, S::Both, C::Smooth, &bounds, join, false);
            same_definitions(&actual, &expected);
            assert!((*actual[0].domain().start() - if join == 2 { 1. } else { 0. }).abs() < 1e-9);
            assert!((*actual[0].domain().end() - 1.75).abs() < 1e-9);
            assert!(
                actual[0]
                    .evaluate(1.75)
                    .unwrap()
                    .is_near(point(7., -7.875), Tolerance::DEFAULT)
            );
        }
    }
    assert_eq!(source, before);
}

#[test]
fn native_boundary_probe_ranges_keep_finite_polynomial_extrapolation_available() {
    let source = NurbsCurve::try_new(
        2,
        vec![point(0., 0.), point(2., 3.), point(4., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    for power in 0..40 {
        let end = 1. + 2_f64.powi(power);
        viboceros_smlib::extend_curve(&source, 0. ..=end)
            .unwrap_or_else(|error| panic!("power={power} end={end}: {error:?}"));
    }
}
