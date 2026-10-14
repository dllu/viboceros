#![cfg(feature = "native")]
use viboceros_geometry::{
    CircularArc3, Curve3, CurveBlendContinuity as G, CurveBlendOptions, LineSegment, NurbsCurve,
    Point3, Tolerance, WeightedPoint3, try_blend_curve,
};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn pair(kind: usize) -> [Curve3; 2] {
    match kind {
        0 => [
            Curve3::Line(
                LineSegment::try_new(point(-1., 0., 0.), point(0., 1., 0.), Tolerance::DEFAULT)
                    .unwrap(),
            ),
            Curve3::Line(
                LineSegment::try_new(point(4., 2., 1.), point(5., 3., 2.), Tolerance::DEFAULT)
                    .unwrap(),
            ),
        ],
        1 => [
            Curve3::Arc(
                CircularArc3::try_from_three_points(
                    point(-2., 0., 0.),
                    point(-2_f64.sqrt(), 2_f64.sqrt(), 0.),
                    point(0., 2., 0.),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ),
            Curve3::Arc(
                CircularArc3::try_from_three_points(
                    point(4., 1., 1.),
                    point(4. + 2_f64.sqrt(), 3. - 2_f64.sqrt(), 1.),
                    point(6., 3., 1.),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ),
        ],
        2 => [0., 5.].map(|offset| {
            Curve3::NurbsCurve(
                NurbsCurve::try_new_rational(
                    3,
                    [
                        ([0., 0., 0.], 1.),
                        ([1., 0.7, 0.2], 0.8),
                        ([2., -0.3, 0.8], 1.4),
                        ([3., 0., 1.], 1.),
                    ]
                    .map(|(p, w)| {
                        WeightedPoint3::try_new(point(p[0] + offset, p[1] + offset, p[2]), w)
                            .unwrap()
                    })
                    .to_vec(),
                    vec![-7., -7., -7., -7., 19., 19., 19., 19.],
                )
                .unwrap(),
            )
        }),
        _ => unreachable!(),
    }
}
fn compare(a: &NurbsCurve, b: &NurbsCurve) {
    assert_eq!(a.degree(), b.degree());
    assert_eq!(a.knots(), b.knots());
    assert_eq!(a.control_points().len(), b.control_points().len());
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert_eq!(a.weight(), 1.);
        assert_eq!(b.weight(), 1.);
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-11);
    }
    for i in 0..=32 {
        let t = *a.domain().start() + i as f64 / 32. * (*a.domain().end() - *a.domain().start());
        assert!(
            a.evaluate(t)
                .unwrap()
                .distance_to(b.evaluate(t).unwrap())
                .unwrap()
                < 1e-11
        );
    }
}

#[test]
fn native_curve_blend_preserves_all_end_picks_continuities_and_handle_policies() {
    for kind in 0..3 {
        let parents = pair(kind);
        let before = parents.clone();
        for first_end in [false, true] {
            for second_end in [false, true] {
                let pick = |curve: &Curve3, end| {
                    if end {
                        curve.as_ref().end_point().unwrap()
                    } else {
                        curve.as_ref().start_point().unwrap()
                    }
                };
                let picks = [pick(&parents[0], first_end), pick(&parents[1], second_end)];
                for first in [G::Position, G::Tangency, G::Curvature] {
                    for second in [G::Position, G::Tangency, G::Curvature] {
                        for (handles, bulges) in [
                            ([None, None], [1., 1.]),
                            ([Some(0.3), Some(0.7)], [1., 1.]),
                            ([None, None], [0.5, 1.8]),
                        ] {
                            let options = CurveBlendOptions {
                                continuity: [first, second],
                                handles,
                                bulges,
                                endpoint_specific: true,
                            };
                            let native = viboceros_smlib::blend_curve(
                                &parents[0],
                                picks[0],
                                &parents[1],
                                picks[1],
                                options,
                                Tolerance::DEFAULT,
                            )
                            .unwrap();
                            let portable = try_blend_curve(
                                &parents[0],
                                picks[0],
                                &parents[1],
                                picks[1],
                                options,
                                Tolerance::DEFAULT,
                            )
                            .unwrap();
                            compare(&native, &portable);
                            assert_eq!(native.control_points()[0].point(), picks[0]);
                            assert_eq!(native.control_points().last().unwrap().point(), picks[1]);
                            for (side, continuity) in [first, second].into_iter().enumerate() {
                                if continuity != G::Curvature {
                                    continue;
                                }
                                let reference = parents[side].as_ref();
                                let at_end = if side == 0 { first_end } else { second_end };
                                let t = if at_end {
                                    *reference.domain().end()
                                } else {
                                    *reference.domain().start()
                                };
                                let expected = reference.curvature_vector(t).unwrap().to_array();
                                let t = if side == 0 {
                                    *native.domain().start()
                                } else {
                                    *native.domain().end()
                                };
                                let actual = Curve3::NurbsCurve(native.clone())
                                    .as_ref()
                                    .curvature_vector(t)
                                    .unwrap()
                                    .to_array();
                                for k in 0..3 {
                                    assert!((expected[k] - actual[k]).abs() < 1e-9);
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(parents, before);
    }
}

#[test]
fn native_endpoint_solver_accepts_high_degree_sources_and_signed_extreme_weight_gauges() {
    let parents = pair(2);
    for gauge in [1e-280, -1e280] {
        let parents = parents.clone().map(|curve| {
            let Curve3::NurbsCurve(curve) = curve else {
                unreachable!()
            };
            let elevated = curve.try_change_degree(18, false).unwrap();
            Curve3::NurbsCurve(
                NurbsCurve::try_new_rational(
                    18,
                    elevated
                        .control_points()
                        .iter()
                        .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                        .collect(),
                    elevated.knots().to_vec(),
                )
                .unwrap(),
            )
        });
        let picks = [
            parents[0].as_ref().end_point().unwrap(),
            parents[1].as_ref().start_point().unwrap(),
        ];
        let options = CurveBlendOptions {
            continuity: [G::Curvature; 2],
            handles: [Some(0.4), Some(0.8)],
            endpoint_specific: true,
            ..Default::default()
        };
        compare(
            &viboceros_smlib::blend_curve(
                &parents[0],
                picks[0],
                &parents[1],
                picks[1],
                options,
                Tolerance::DEFAULT,
            )
            .unwrap(),
            &try_blend_curve(
                &parents[0],
                picks[0],
                &parents[1],
                picks[1],
                options,
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
    }
}
