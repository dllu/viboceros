#![cfg(feature = "native")]
use serde_json::{Value, json};
use viboceros_geometry::{NurbsCurve, NurbsSurface};
use viboceros_smlib::{change_curve_degree, change_surface_degree};
#[path = "support/nurbs.rs"]
mod support;
use support::{compare, control_values, curve_value, knots, points, surface_value};

#[test]
fn separated_discontinuous_elevation_preserves_independent_bezier_controls_and_source() {
    use viboceros_geometry::Point3;
    let source = NurbsCurve::try_new(
        2,
        [
            [0., 0., 0.],
            [1., 1., 0.],
            [2., 0., 0.],
            [4., 0., 0.],
            [5., -1., 0.],
            [6., 0., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    let elevated = change_curve_degree(&source, 4, false).unwrap();
    assert_eq!(elevated.degree(), 4);
    assert_eq!(
        elevated.knots(),
        &[
            0., 0., 0., 0., 0., 0.5, 0.5, 0.5, 0.5, 0.5, 1., 1., 1., 1., 1.
        ]
    );
    let expected = [
        [0., 0., 0.],
        [0.5, 0.5, 0.],
        [1., 2. / 3., 0.],
        [1.5, 0.5, 0.],
        [2., 0., 0.],
        [4., 0., 0.],
        [4.5, -0.5, 0.],
        [5., -2. / 3., 0.],
        [5.5, -0.5, 0.],
        [6., 0., 0.],
    ];
    for (a, b) in elevated.control_points().iter().zip(expected) {
        assert!((a.weight() - 1.).abs() < 1e-14);
        for (a, b) in a.point().to_array().into_iter().zip(b) {
            assert!((a - b).abs() < 1e-14);
        }
    }
    for t in [0., 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.] {
        assert!(
            elevated
                .evaluate(t)
                .unwrap()
                .distance_to(source.evaluate(t).unwrap())
                .unwrap()
                < 1e-13
        );
    }
    assert_eq!(source, before);
}

#[test]
fn separated_rational_elevation_covers_many_blocks_all_native_degrees_and_tensor_axes() {
    use viboceros_geometry::{Point3, WeightedPoint3};
    for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            let source = NurbsCurve::try_new_rational(
                2,
                (0..9)
                    .map(|i| {
                        WeightedPoint3::try_new(
                            Point3::try_new(
                                i as f64,
                                ((i % 3) as f64 - 1.).powi(2),
                                i as f64 * 0.1,
                            )
                            .unwrap(),
                            gauge * [1., 0.5, 1.5][i % 3],
                        )
                        .unwrap()
                    })
                    .collect(),
                [0., 0., 0., 0.25, 0.25, 0.25, 0.75, 0.75, 0.75, 1., 1., 1.]
                    .map(|k| k * chart)
                    .to_vec(),
            )
            .unwrap();
            let before = source.clone();
            for degree in [3, 4, 7, 11, 17] {
                let result = change_curve_degree(&source, degree, false).unwrap();
                assert_eq!(
                    result.knots(),
                    viboceros_geometry::degree_change_knots(2, degree, false, source.knots())
                        .unwrap()
                );
                assert_eq!(result.control_points().len(), 3 * (degree + 1));
                for f in [0., 0.125, 0.25, 0.5, 0.75, 0.875, 1.] {
                    assert!(
                        result
                            .evaluate(f * chart)
                            .unwrap()
                            .distance_to(source.evaluate(f * chart).unwrap())
                            .unwrap()
                            < 1e-10
                    );
                }
            }
            assert_eq!(source, before);
        }
    }
    let net = (0..6)
        .flat_map(|j| {
            (0..6).map(move |i| {
                WeightedPoint3::try_new(
                    Point3::try_new(i as f64, j as f64, (i * j) as f64 * 0.2).unwrap(),
                    1. + (i + j) as f64 * 0.1,
                )
                .unwrap()
            })
        })
        .collect();
    let knots = vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.];
    let source = NurbsSurface::try_new_rational(2, 2, 6, 6, net, knots.clone(), knots).unwrap();
    let before = source.clone();
    let result = change_surface_degree(&source, 4, 3, false).unwrap();
    assert_eq!(
        (
            result.control_point_count_u(),
            result.control_point_count_v()
        ),
        (10, 8)
    );
    for j in 0..=16 {
        for i in 0..=16 {
            let (u, v) = (i as f64 / 16., j as f64 / 16.);
            assert!(
                result
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(source.evaluate(u, v).unwrap())
                    .unwrap()
                    < 1e-10
            );
        }
    }
    assert_eq!(source, before);
}

#[test]
fn separated_elevation_preserves_internal_knots_and_both_one_sided_limits() {
    use viboceros_geometry::{CurveRef, ParameterSide, Point3, WeightedPoint3};
    let source = NurbsCurve::try_new_rational(
        2,
        (0..9)
            .map(|i| {
                WeightedPoint3::try_new(
                    Point3::try_new(i as f64, (i % 4) as f64 - 2., (i * i) as f64 * 0.1).unwrap(),
                    1. + i as f64 * 0.125,
                )
                .unwrap()
            })
            .collect(),
        vec![0., 0., 0., 0.2, 0.35, 0.5, 0.5, 0.5, 0.8, 1., 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    let result = change_curve_degree(&source, 7, false).unwrap();
    assert_eq!(
        result.knots(),
        viboceros_geometry::degree_change_knots(2, 7, false, source.knots()).unwrap()
    );
    for i in 0..=256 {
        let t = i as f64 / 256.;
        assert!(
            result
                .evaluate(t)
                .unwrap()
                .distance_to(source.evaluate(t).unwrap())
                .unwrap()
                < 1e-10
        );
    }
    for side in [ParameterSide::Left, ParameterSide::Right] {
        let a = CurveRef::NurbsCurve(&source)
            .evaluate_on_side(0.5, side)
            .unwrap();
        let b = CurveRef::NurbsCurve(&result)
            .evaluate_on_side(0.5, side)
            .unwrap();
        assert!(a.distance_to(b).unwrap() < 1e-12);
    }
    assert_eq!(source, before);
}

#[test]
fn exact_elevation_retains_short_spans_extreme_charts_and_weight_gauges() {
    use viboceros_geometry::{Point3, WeightedPoint3};
    for scale in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            let close = 0.5 + 1e-14;
            let source = NurbsCurve::try_new_rational(
                2,
                [
                    [2., 0., 0.],
                    [3., 0., 0.],
                    [4., 0., 0.],
                    [4., 0., 10.],
                    [5., 0., 0.],
                    [6., 0., 0.],
                    [7., 0., 0.],
                ]
                .into_iter()
                .zip([1., 0.75, 1., 0.5, 1., 0.75, 1.])
                .map(|(p, w)| {
                    WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w * gauge).unwrap()
                })
                .collect(),
                [0., 0., 0., 0.5, 0.5, close, close, 1., 1., 1.]
                    .map(|k| k * scale)
                    .to_vec(),
            )
            .unwrap();
            let result = change_curve_degree(&source, 4, false).unwrap();
            assert_eq!(result.domain(), source.domain());
            for t in [0., 0.25, 0.5, (0.5 + close) * 0.5, close, 0.75, 1.] {
                assert!(
                    source
                        .evaluate(t * scale)
                        .unwrap()
                        .distance_to(result.evaluate(t * scale).unwrap())
                        .unwrap()
                        < 1e-10
                );
            }
            assert_eq!(
                result.knots(),
                viboceros_geometry::degree_change_knots(2, 4, false, source.knots()).unwrap()
            );
        }
    }
}

#[test]
fn native_degree_changes_match_all_curve_and_surface_modes_without_source_mutation() {
    for family in ["curve", "surface"] {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
        let fixture: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join("fixtures")
                    .join(format!("{family}_change_degree.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let reference: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join("observations")
                    .join(format!("{family}_change_degree.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(fixture["operations"].as_array().unwrap().len(), 5);
        assert_eq!(reference["results"].as_array().unwrap().len(), 5);
        for (op, record) in fixture["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(reference["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], record["id"]);
            let deformable = op["deformable"].as_bool().unwrap_or(false);
            if family == "curve" {
                let c = &op["curve"];
                let source = NurbsCurve::try_new_rational(
                    c["degree"].as_u64().unwrap() as usize,
                    points(&c["control_points"]),
                    knots(&c["knots"]),
                )
                .unwrap();
                let before = source.clone();
                let result = change_curve_degree(
                    &source,
                    op["degree"].as_u64().unwrap() as usize,
                    deformable,
                )
                .unwrap();
                let actual = curve_value(&result);
                compare(&actual, &record["value"], op["id"].as_str().unwrap());
                assert_eq!(source, before);
            } else {
                let source = NurbsSurface::try_new_rational(
                    op["degree_u"].as_u64().unwrap() as usize,
                    op["degree_v"].as_u64().unwrap() as usize,
                    op["control_point_count_u"].as_u64().unwrap() as usize,
                    op["control_point_count_v"].as_u64().unwrap() as usize,
                    points(&op["control_points"]),
                    knots(&op["knots_u"]),
                    knots(&op["knots_v"]),
                )
                .unwrap();
                let before = source.clone();
                let result = change_surface_degree(
                    &source,
                    op["desired_degree_u"].as_u64().unwrap() as usize,
                    op["desired_degree_v"].as_u64().unwrap() as usize,
                    deformable,
                )
                .unwrap();
                let actual = surface_value(&result);
                compare(&actual, &record["value"], op["id"].as_str().unwrap());
                assert_eq!(source, before);
            }
        }
    }
}

#[test]
fn curve_modes_agree_with_existing_independent_geometry_and_preserve_noops() {
    let source = NurbsCurve::try_clamped_uniform(
        3,
        points(&json!([
            {"point":[-2.,1.,0.],"weight":1.},{"point":[0.,3.,1.],"weight":0.5},
            {"point":[2.,-2.,-1.],"weight":2.},{"point":[4.,0.,2.],"weight":1.}
        ]))
        .into_iter()
        .map(|p| p.point())
        .collect(),
    )
    .unwrap();
    for (degree, deformable) in [(5, false), (2, false), (5, true), (3, true)] {
        let actual = change_curve_degree(&source, degree, deformable).unwrap();
        let expected = source.try_change_degree(degree, deformable).unwrap();
        assert_eq!(actual.knots(), expected.knots());
        for i in 0..=64 {
            let t = source.parameter_at(i as f64 / 64.).unwrap();
            assert!(
                actual
                    .evaluate(t)
                    .unwrap()
                    .distance_to(expected.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-10
            );
        }
    }
    assert_eq!(
        change_curve_degree(&source, source.degree(), false).unwrap(),
        source
    );
}

#[test]
fn rational_interpolation_preserves_extreme_charts_and_common_weight_gauges() {
    use viboceros_geometry::{Point3, WeightedPoint3};
    for scale in [2_f64.powi(-360), 1., 2_f64.powi(360), 2_f64.powi(1022)] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            let source = NurbsCurve::try_new_rational(
                3,
                [
                    [-2., 1., 0.],
                    [0., 3., 1.],
                    [2., -2., -1.],
                    [4., 0., 2.],
                    [5., 1., 0.],
                ]
                .into_iter()
                .zip([1., 0.5, 2., 1., 0.75])
                .map(|(p, w)| {
                    WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w * gauge).unwrap()
                })
                .collect(),
                [0., 0., 0., 0., 0.5, 1., 1., 1., 1.]
                    .map(|k| k * scale)
                    .to_vec(),
            )
            .unwrap();
            for (degree, deformable) in [(2, false), (5, true)] {
                let actual = change_curve_degree(&source, degree, deformable).unwrap();
                let expected = source.try_change_degree(degree, deformable).unwrap();
                assert_eq!(actual.knots(), expected.knots());
                compare(
                    &control_values(actual.control_points()),
                    &control_values(expected.control_points()),
                    "rational degree-change controls",
                );
                for i in 0..=64 {
                    let t = source.parameter_at(i as f64 / 64.).unwrap();
                    assert!(
                        actual
                            .evaluate(t)
                            .unwrap()
                            .distance_to(expected.evaluate(t).unwrap())
                            .unwrap()
                            < 1e-10
                    );
                }
            }
        }
    }
}

#[test]
fn rational_tensor_modes_agree_with_independent_geometry_and_preserve_noops() {
    use viboceros_geometry::{Point3, WeightedPoint3};
    let source = NurbsSurface::try_new_rational(
        3,
        2,
        4,
        3,
        (0..3)
            .flat_map(|v| {
                (0..4).map(move |u| {
                    WeightedPoint3::try_new(
                        Point3::try_new(u as f64, v as f64, (u * u + v * v) as f64 / 4.).unwrap(),
                        1. + (u + v) as f64 / 8.,
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![-2., -2., -2., -2., 3., 3., 3., 3.],
        vec![4., 4., 4., 9., 9., 9.],
    )
    .unwrap();
    for (u, v, deformable) in [(2, 4, false), (5, 1, true), (3, 2, true)] {
        let actual = change_surface_degree(&source, u, v, deformable).unwrap();
        let expected = source.try_change_degree(u, v, deformable).unwrap();
        assert_eq!(actual.knots_u(), expected.knots_u());
        assert_eq!(actual.knots_v(), expected.knots_v());
        compare(
            &control_values(actual.control_points()),
            &control_values(expected.control_points()),
            "rational tensor degree-change controls",
        );
        for i in 0..=16 {
            for j in 0..=16 {
                let u = -2. + 5. * i as f64 / 16.;
                let v = 4. + 5. * j as f64 / 16.;
                assert!(
                    actual
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(expected.evaluate(u, v).unwrap())
                        .unwrap()
                        < 1e-10
                );
            }
        }
    }
    assert_eq!(change_surface_degree(&source, 3, 2, true).unwrap(), source);
}
