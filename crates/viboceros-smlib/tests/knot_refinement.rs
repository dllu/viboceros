#![cfg(feature = "native")]
use serde_json::{Value, json};
use viboceros_geometry::{NurbsCurve, NurbsSurface, Point3, SurfaceKnotDirection, WeightedPoint3};
use viboceros_smlib::{
    insert_curve_knot, insert_surface_knot, make_curve_non_periodic, make_surface_non_periodic,
};

#[test]
fn nonperiodic_conversion_preserves_full_curve_and_tensor_geometry_and_noops() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/make_non_periodic.json"
    ))
    .unwrap();
    let t = SurfaceKnotDirection::Both;
    for op in fixture["operations"].as_array().unwrap() {
        if let Some(c) = op.get("curve") {
            let source = NurbsCurve::try_new_rational(
                c["degree"].as_u64().unwrap() as usize,
                points(&c["control_points"]),
                knots(&c["knots"]),
            )
            .unwrap();
            for gauge in [1., 1e-300, 1e300, -1e300] {
                let source = NurbsCurve::try_new_rational(
                    source.degree(),
                    source
                        .control_points()
                        .iter()
                        .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                        .collect(),
                    source.knots().to_vec(),
                )
                .unwrap();
                let before = source.clone();
                let result = make_curve_non_periodic(&source).unwrap();
                assert!(!result.is_periodic());
                assert_eq!(source, before);
                assert_eq!(result.domain(), source.domain());
                assert_eq!(make_curve_non_periodic(&result).unwrap(), result);
                for i in 0..=64 {
                    let p = source.parameter_at(i as f64 / 64.).unwrap();
                    assert!(
                        source
                            .evaluate(p)
                            .unwrap()
                            .distance_to(result.evaluate(p).unwrap())
                            .unwrap()
                            < 1e-10
                    );
                }
            }
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
            for direction in [SurfaceKnotDirection::U, SurfaceKnotDirection::V, t] {
                let result = make_surface_non_periodic(&source, direction).unwrap();
                if matches!(
                    direction,
                    SurfaceKnotDirection::U | SurfaceKnotDirection::Both
                ) {
                    assert!(!result.is_periodic_u());
                }
                if matches!(
                    direction,
                    SurfaceKnotDirection::V | SurfaceKnotDirection::Both
                ) {
                    assert!(!result.is_periodic_v());
                }
                assert_eq!(result.domain_u(), source.domain_u());
                assert_eq!(result.domain_v(), source.domain_v());
                assert_eq!(source, before);
                assert_eq!(
                    make_surface_non_periodic(&result, direction).unwrap(),
                    result
                );
                for i in 0..=16 {
                    for j in 0..=16 {
                        let (u, v) = (
                            source.parameter_at_u(i as f64 / 16.).unwrap(),
                            source.parameter_at_v(j as f64 / 16.).unwrap(),
                        );
                        assert!(
                            source
                                .evaluate(u, v)
                                .unwrap()
                                .distance_to(result.evaluate(u, v).unwrap())
                                .unwrap()
                                < 1e-10
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn nonperiodic_conversion_matches_complete_recorded_rhino_definitions() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/make_non_periodic.json"
    ))
    .unwrap();
    let reference: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/rhino_oracle/observations/make_non_periodic.json"
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(reference["results"].as_array().unwrap().len(), 4);
    for (op, record) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], record["id"]);
        let expected = &record["value"];
        if let Some(c) = op.get("curve") {
            let source = NurbsCurve::try_new_rational(
                c["degree"].as_u64().unwrap() as usize,
                points(&c["control_points"]),
                knots(&c["knots"]),
            )
            .unwrap();
            let result = make_curve_non_periodic(&source).unwrap();
            let actual = json!({"degree":result.degree(),"domain":[*result.domain().start(),*result.domain().end()],"knots":result.knots(),"control_points":control_values(result.control_points()),"closed":result.is_closed().unwrap(),"periodic":result.is_periodic()});
            compare(&actual, expected, op["id"].as_str().unwrap());
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
            let result = make_surface_non_periodic(&source, SurfaceKnotDirection::Both).unwrap();
            let actual = json!({"degree":[result.degree_u(),result.degree_v()],"domain_u":[*result.domain_u().start(),*result.domain_u().end()],"domain_v":[*result.domain_v().start(),*result.domain_v().end()],"knots_u":result.knots_u(),"knots_v":result.knots_v(),"control_count":[result.control_point_count_u(),result.control_point_count_v()],"control_points":control_values(result.control_points()),"periodic_u":result.is_periodic_u(),"periodic_v":result.is_periodic_v()});
            compare(&actual, expected, op["id"].as_str().unwrap());
        }
    }
}

#[path = "support/nurbs.rs"]
mod support;
use support::{compare, control_values, knots, points};

#[test]
fn native_curve_refinement_matches_complete_rhino_definitions_and_preserves_source() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_insert_knot.json"
    ))
    .unwrap();
    let reference: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/rhino_oracle/observations/curve_insert_knot.json"
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(reference["results"].as_array().unwrap().len(), 8);
    let mut failures = Vec::new();
    for (op, record) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], record["id"]);
        let c = &op["curve"];
        let source = NurbsCurve::try_new_rational(
            c["degree"].as_u64().unwrap() as usize,
            points(&c["control_points"]),
            knots(&c["knots"]),
        )
        .unwrap();
        let before = source.clone();
        match insert_curve_knot(
            &source,
            op["parameter"].as_f64().unwrap(),
            op["multiplicity"].as_u64().unwrap() as usize,
        ) {
            Ok(result) => {
                let e = &record["value"];
                for (key, value) in [
                    ("degree", json!(result.degree())),
                    ("knots", json!(result.knots())),
                    ("control_points", control_values(result.control_points())),
                    ("periodic", json!(result.is_periodic())),
                    ("closed", json!(result.is_closed().unwrap())),
                    (
                        "domain",
                        json!([*result.domain().start(), *result.domain().end()]),
                    ),
                ] {
                    compare(&value, &e[key], &format!("{}/{key}", op["id"]));
                }
                for i in 0..=64 {
                    let t = source.parameter_at(i as f64 / 64.).unwrap();
                    assert!(
                        source
                            .evaluate(t)
                            .unwrap()
                            .distance_to(result.evaluate(t).unwrap())
                            .unwrap()
                            < 1e-9,
                        "{}",
                        op["id"]
                    );
                }
            }
            Err(error) => failures.push(format!("{}: {error}", op["id"])),
        }
        assert_eq!(source, before);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn insertion_preserves_short_spans_parameter_charts_and_extreme_common_gauges() {
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
            let parameter = ((0.5 + close) * 0.5) * scale;
            let result = insert_curve_knot(&source, parameter, 2).unwrap();
            assert_eq!(result.domain(), source.domain());
            assert_eq!(
                result.control_points().len(),
                source.control_points().len() + 2
            );
            for v in [0., 0.25, 0.5, (0.5 + close) * 0.5, close, 0.75, 1.] {
                assert!(
                    source
                        .evaluate(v * scale)
                        .unwrap()
                        .distance_to(result.evaluate(v * scale).unwrap())
                        .unwrap()
                        < 1e-10
                );
            }
        }
    }
}

#[test]
fn unclamped_tensor_endpoints_in_both_axes_preserve_the_complete_image() {
    let source = NurbsSurface::try_new(
        2,
        2,
        4,
        4,
        (0..4)
            .flat_map(|j| {
                (0..4).map(move |i| Point3::try_new(i as f64, j as f64, (i * j) as f64).unwrap())
            })
            .collect(),
        vec![-2., -1., 0., 0.8, 2., 3., 4.],
        vec![-2., -1., 0., 0.8, 2., 3., 4.],
    )
    .unwrap();
    for direction in [SurfaceKnotDirection::U, SurfaceKnotDirection::V] {
        for at in [0., 2.] {
            let result = insert_surface_knot(&source, direction, at, 2).unwrap();
            let portable = match direction {
                SurfaceKnotDirection::U => source.try_insert_knot_u(at, 2).unwrap(),
                SurfaceKnotDirection::V => source.try_insert_knot_v(at, 2).unwrap(),
                SurfaceKnotDirection::Both => unreachable!(),
            };
            assert_eq!(result.knots_u(), portable.knots_u());
            assert_eq!(result.knots_v(), portable.knots_v());
            for i in 0..=16 {
                for j in 0..=16 {
                    let (u, v) = (i as f64 / 8., j as f64 / 8.);
                    assert!(
                        source
                            .evaluate(u, v)
                            .unwrap()
                            .distance_to(result.evaluate(u, v).unwrap())
                            .unwrap()
                            < 1e-11
                    );
                }
            }
        }
    }
}

#[test]
fn native_tensor_refinement_matches_complete_rhino_definitions_and_periodic_policy() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_insert_knot.json"
    ))
    .unwrap();
    let reference: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/rhino_oracle/observations/surface_insert_knot.json"
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(reference["results"].as_array().unwrap().len(), 5);
    let mut failures = Vec::new();
    for (op, record) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], record["id"]);
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
        let axis = if op["direction"] == "u" {
            SurfaceKnotDirection::U
        } else {
            SurfaceKnotDirection::V
        };
        match insert_surface_knot(
            &source,
            axis,
            op["parameter"].as_f64().unwrap(),
            op["multiplicity"].as_u64().unwrap() as usize,
        ) {
            Ok(result) => {
                let e = &record["value"];
                for (key, value) in [
                    ("degree", json!([result.degree_u(), result.degree_v()])),
                    (
                        "control_count",
                        json!([
                            result.control_point_count_u(),
                            result.control_point_count_v()
                        ]),
                    ),
                    ("knots_u", json!(result.knots_u())),
                    ("knots_v", json!(result.knots_v())),
                    (
                        "domain_u",
                        json!([*result.domain_u().start(), *result.domain_u().end()]),
                    ),
                    (
                        "domain_v",
                        json!([*result.domain_v().start(), *result.domain_v().end()]),
                    ),
                    ("control_points", control_values(result.control_points())),
                    ("periodic_u", json!(result.is_periodic_u())),
                    ("periodic_v", json!(result.is_periodic_v())),
                ] {
                    compare(&value, &e[key], &format!("{}/{key}", op["id"]));
                }
                for i in 0..=16 {
                    for j in 0..=16 {
                        let (u, v) = (
                            source.parameter_at_u(i as f64 / 16.).unwrap(),
                            source.parameter_at_v(j as f64 / 16.).unwrap(),
                        );
                        assert!(
                            source
                                .evaluate(u, v)
                                .unwrap()
                                .distance_to(result.evaluate(u, v).unwrap())
                                .unwrap()
                                < 1e-9,
                            "{}",
                            op["id"]
                        );
                    }
                }
            }
            Err(error) => failures.push(format!("{}: {error}", op["id"])),
        }
        assert_eq!(source, before);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
