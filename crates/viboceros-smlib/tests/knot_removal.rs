#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{NurbsCurve, NurbsSurface, Point3, SurfaceKnotDirection, WeightedPoint3};
use viboceros_smlib::{remove_curve_knot, remove_surface_knot};
#[path = "support/nurbs.rs"]
mod support;
use support::{compare, curve_value, knots, points, surface_value};

#[test]
fn native_removal_matches_all_raw_rhino_curve_and_tensor_cases_without_mutating_sources() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/remove_knot.json")).unwrap(),
    )
    .unwrap();
    let reference: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/remove_knot.json")).unwrap(),
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
        let parameter = op["parameter"].as_f64().unwrap();
        let actual = if op["op"] == "curve_remove_knot_geometry" {
            let c = &op["curve"];
            let source = NurbsCurve::try_new_rational(
                c["degree"].as_u64().unwrap() as usize,
                points(&c["control_points"]),
                knots(&c["knots"]),
            )
            .unwrap();
            let before = source.clone();
            let result = remove_curve_knot(&source, parameter).unwrap();
            assert_eq!(source, before);
            curve_value(&result)
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
            let direction = if op["direction"] == "u" {
                SurfaceKnotDirection::U
            } else {
                SurfaceKnotDirection::V
            };
            let before = source.clone();
            let result = remove_surface_knot(&source, direction, parameter).unwrap();
            assert_eq!(source, before);
            surface_value(&result)
        };
        compare(&actual, &record["value"], op["id"].as_str().unwrap());
    }
}

#[test]
fn unclamped_rational_removal_matches_portable_controls_at_extreme_charts_and_gauges() {
    for scale in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            let source = NurbsCurve::try_new_rational(
                2,
                [
                    [0., 0., 0.],
                    [1., 3., 1.],
                    [4., -1., 1.],
                    [6., 2., 0.],
                    [8., 0., 1.],
                ]
                .into_iter()
                .zip([1., 0.5, 2., 0.75, 1.])
                .map(|(p, w)| {
                    WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w * gauge).unwrap()
                })
                .collect(),
                [-2., -1., 0., 0.3, 0.7, 1., 2., 3.]
                    .map(|k| k * scale)
                    .to_vec(),
            )
            .unwrap();
            let before = source.clone();
            let start = viboceros_smlib::insert_curve_knot(&source, 0., 3).unwrap();
            let clamped = viboceros_smlib::insert_curve_knot(&start, scale, 3).unwrap();
            assert_eq!(
                clamped.knots(),
                [0., 0., 0., 0.3, 0.7, 1., 1., 1.].map(|k| k * scale)
            );
            let expected = source.try_remove_knot_near(0.3 * scale).unwrap();
            let actual = remove_curve_knot(&source, 0.3 * scale).unwrap();
            compare(
                &curve_value(&actual),
                &curve_value(&expected),
                "unclamped rational removal",
            );
            for i in 0..=64 {
                let t = source.parameter_at(i as f64 / 64.).unwrap();
                assert!(
                    source
                        .evaluate(t)
                        .unwrap()
                        .distance_to(clamped.evaluate(t).unwrap())
                        .unwrap()
                        < 1e-10
                );
                assert!(
                    actual
                        .evaluate(t)
                        .unwrap()
                        .distance_to(expected.evaluate(t).unwrap())
                        .unwrap()
                        < 1e-10
                );
            }
            assert_eq!(source, before);
        }
    }
}

#[test]
fn tensor_removal_uses_whole_net_periodicity_and_preserves_the_other_basis() {
    let controls = [
        [0., 0., 0.],
        [2., 0., 1.],
        [1., 2., 0.],
        [0., 0., 0.],
        [2., 0., 1.],
        [0., 0., 2.],
        [2., 0., 3.],
        [1., 2., 2.],
        [0., 0., 2.],
        [4., 0., 3.],
    ];
    let source = NurbsSurface::try_new(
        2,
        1,
        5,
        2,
        controls
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![-2., -1., 0., 1., 2., 3., 4., 5.],
        vec![4., 4., 9., 9.],
    )
    .unwrap();
    assert!(!source.is_periodic_u());
    let before = source.clone();
    let actual = remove_surface_knot(&source, SurfaceKnotDirection::U, 1.5).unwrap();
    let expected = source.try_remove_knot_u_near(1.5).unwrap();
    assert_eq!(
        source
            .knot_removal_candidate(SurfaceKnotDirection::U, 1.5)
            .unwrap(),
        2.
    );
    assert_eq!(actual.knots_v(), source.knots_v());
    compare(
        &surface_value(&actual),
        &surface_value(&expected),
        "whole-net periodicity",
    );
    for i in 0..=16 {
        for j in 0..=16 {
            let u = 3. * i as f64 / 16.;
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
    assert!(remove_surface_knot(&source, SurfaceKnotDirection::Both, 1.5).is_err());
    assert_eq!(source, before);
}

#[test]
fn removal_interpolation_retains_clamped_endpoints_next_to_a_short_span() {
    let source = NurbsCurve::try_new_rational(
        2,
        [
            [0., 0., 0.],
            [1., 3., 1.],
            [4., -1., 1.],
            [6., 2., 0.],
            [8., 0., 1.],
        ]
        .into_iter()
        .zip([1., 0.5, 2., 0.75, 1.])
        .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
        .collect(),
        vec![0., 0., 0., 1e-14, 0.7, 1., 1., 1.],
    )
    .unwrap();
    let expected = source.try_remove_knot_near(0.7).unwrap();
    let actual = remove_curve_knot(&source, 0.7).unwrap();
    compare(
        &curve_value(&actual),
        &curve_value(&expected),
        "short first span",
    );
}

#[test]
fn repeated_unclamped_endpoint_rejection_retains_the_requested_parameter() {
    use viboceros_geometry::GeometryError;
    let source = NurbsCurve::try_new(
        2,
        (0..5)
            .map(|i| Point3::try_new(i as f64, 0., 0.).unwrap())
            .collect(),
        vec![-2., -1., 0., 0.3, 1., 1., 2., 3.],
    )
    .unwrap();
    let expected = GeometryError::NoRemovableKnot { parameter: 0.98 };
    assert_eq!(source.knot_removal_candidate(0.98), Err(expected.clone()));
    assert_eq!(source.try_remove_knot_near(0.98), Err(expected.clone()));
    let before = source.clone();
    match remove_curve_knot(&source, 0.98) {
        Err(viboceros_smlib::Error::Geometry(error)) => assert_eq!(error, expected),
        other => panic!("expected original geometry error, got {other:?}"),
    }
    assert_eq!(source, before);
}

#[test]
fn decimal_endpoint_interpolation_uses_exact_clamped_samples() {
    for offset in [0., -0.1] {
        let source = NurbsCurve::try_new(
            3,
            [
                [0., 0., 0.],
                [2., 4., 1.],
                [5., -1., 2.],
                [7., 3., -1.],
                [9., 1., 0.],
                [12., 5., 2.],
                [15., -2., 1.],
            ]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
            [0., 0., 0., 0., 1., 3., 5., 8., 8., 8., 8.]
                .map(|k| k * 0.0125 + offset)
                .to_vec(),
        )
        .unwrap();
        let parameter = 3. * 0.0125 + offset;
        let expected = source.try_remove_knot_near(parameter).unwrap();
        let actual = remove_curve_knot(&source, parameter).unwrap();
        compare(
            &curve_value(&actual),
            &curve_value(&expected),
            "decimal endpoint samples",
        );
    }
}
