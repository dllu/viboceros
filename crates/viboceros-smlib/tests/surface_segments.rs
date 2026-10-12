#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    Frame3, GeometryError, NurbsSurface, Point3, SurfaceKnotDirection, Tolerance, Vector3,
    WeightedPoint3,
};
use viboceros_smlib::{Error, split_surface, trim_surface, trim_surface_direction};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_trim_geometry.json"
    ))
    .unwrap()
}
fn source(op: &Value) -> NurbsSurface {
    let s = &op["surface"];
    let mut source = NurbsSurface::try_new_rational(
        s["degree_u"].as_u64().unwrap() as usize,
        s["degree_v"].as_u64().unwrap() as usize,
        s["control_point_count_u"].as_u64().unwrap() as usize,
        s["control_point_count_v"].as_u64().unwrap() as usize,
        support::points(&s["control_points"]),
        support::knots(&s["knots_u"]),
        support::knots(&s["knots_v"]),
    )
    .unwrap();
    if let Some(u) = s.get("domain_u") {
        let v = &s["domain_v"];
        source = source
            .try_reparameterized(
                u[0].as_f64().unwrap()..=u[1].as_f64().unwrap(),
                v[0].as_f64().unwrap()..=v[1].as_f64().unwrap(),
            )
            .unwrap();
    }
    source
}

fn compare(actual: &NurbsSurface, expected: &NurbsSurface) {
    assert_eq!(actual.degree_u(), expected.degree_u());
    assert_eq!(actual.degree_v(), expected.degree_v());
    assert_eq!(actual.knots_u(), expected.knots_u());
    assert_eq!(actual.knots_v(), expected.knots_v());
    assert_eq!(
        actual.control_point_count_u(),
        expected.control_point_count_u()
    );
    assert_eq!(
        actual.control_point_count_v(),
        expected.control_point_count_v()
    );
    assert_eq!(actual.is_periodic_u(), expected.is_periodic_u());
    assert_eq!(actual.is_periodic_v(), expected.is_periodic_v());
    for (a, e) in actual
        .control_points()
        .iter()
        .zip(expected.control_points())
    {
        assert!(a.point().distance_to(e.point()).unwrap() < 1e-10);
        assert!((a.weight() / e.weight() - 1.).abs() < 1e-12);
    }
    for i in 0..=8 {
        for j in 0..=8 {
            let (u, v) = (
                actual.parameter_at_u(i as f64 / 8.).unwrap(),
                actual.parameter_at_v(j as f64 / 8.).unwrap(),
            );
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

#[test]
fn recorded_rhino_trims_match_with_the_whole_domain_outer_padding_difference_retained() {
    let fixture = fixture();
    let reference: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_trim_geometry.json"
    ))
    .unwrap();
    assert_eq!(reference["results"].as_array().unwrap().len(), 11);
    for (op, record) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], record["id"]);
        let source = source(op);
        let actual = trim_surface(
            &source,
            op["u"][0].as_f64().unwrap()..=op["u"][1].as_f64().unwrap(),
            op["v"][0].as_f64().unwrap()..=op["v"][1].as_f64().unwrap(),
        )
        .unwrap();
        let e = &record["value"];
        if op["id"] == "trim-whole-inactive-padding" {
            assert_eq!(actual, source);
            let expected = NurbsSurface::try_new_rational(
                e["degree"][0].as_u64().unwrap() as usize,
                e["degree"][1].as_u64().unwrap() as usize,
                e["control_count"][0].as_u64().unwrap() as usize,
                e["control_count"][1].as_u64().unwrap() as usize,
                support::points(&e["control_points"]),
                support::knots(&e["knots_u"]),
                support::knots(&e["knots_v"]),
            )
            .unwrap();
            for (a, e) in [
                (actual.knots_u(), expected.knots_u()),
                (actual.knots_v(), expected.knots_v()),
            ] {
                assert_eq!(a[1..a.len() - 1], e[1..e.len() - 1]);
                assert_eq!((a[0], e[0]), (-2., -1.));
                assert_eq!((*a.last().unwrap(), *e.last().unwrap()), (4., 3.));
            }
            assert_eq!(actual.control_points(), expected.control_points());
            for i in 0..=8 {
                for j in 0..=8 {
                    let (u, v) = (
                        actual.parameter_at_u(i as f64 / 8.).unwrap(),
                        actual.parameter_at_v(j as f64 / 8.).unwrap(),
                    );
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
        } else {
            support::compare(
                &support::surface_value(&actual),
                e,
                op["id"].as_str().unwrap(),
            );
        }
    }
}

#[test]
fn native_rectangles_preserve_rational_definitions_original_loci_and_sources() {
    let fixture = fixture();
    assert_eq!(fixture["operations"].as_array().unwrap().len(), 11);
    for op in fixture["operations"].as_array().unwrap() {
        let source = source(op);
        let before = source.clone();
        let u = op["u"][0].as_f64().unwrap()..=op["u"][1].as_f64().unwrap();
        let v = op["v"][0].as_f64().unwrap()..=op["v"][1].as_f64().unwrap();
        let actual = trim_surface(&source, u.clone(), v.clone()).unwrap();
        compare(&actual, &source.try_trimmed(u, v).unwrap());
        for i in 0..=8 {
            for j in 0..=8 {
                let (u, v) = (
                    actual.parameter_at_u(i as f64 / 8.).unwrap(),
                    actual.parameter_at_v(j as f64 / 8.).unwrap(),
                );
                assert!(
                    actual
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
}

#[test]
fn native_surface_segments_preserve_binary_charts_and_common_or_independent_row_gauges() {
    let source = source(&fixture()["operations"][0]);
    for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            let source = NurbsSurface::try_new_rational(
                2,
                2,
                4,
                4,
                source
                    .control_points()
                    .iter()
                    .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                    .collect(),
                source.knots_u().iter().map(|k| k * chart).collect(),
                source.knots_v().iter().map(|k| k * chart).collect(),
            )
            .unwrap();
            let actual = trim_surface(
                &source,
                0.25 * chart..=1.6 * chart,
                0.2 * chart..=1.7 * chart,
            )
            .unwrap();
            compare(
                &actual,
                &source
                    .try_trimmed(0.25 * chart..=1.6 * chart, 0.2 * chart..=1.7 * chart)
                    .unwrap(),
            );
        }
    }
    let source = NurbsSurface::try_new_rational(
        2,
        2,
        4,
        4,
        source
            .control_points()
            .iter()
            .enumerate()
            .map(|(i, p)| {
                WeightedPoint3::try_new(
                    p.point(),
                    p.weight() * if i / 4 % 2 == 0 { 1e-280 } else { 1e280 },
                )
                .unwrap()
            })
            .collect(),
        source.knots_u().to_vec(),
        source.knots_v().to_vec(),
    )
    .unwrap();
    compare(
        &trim_surface_direction(&source, SurfaceKnotDirection::U, 0.25..=1.6).unwrap(),
        &source.try_trimmed_u(0.25..=1.6).unwrap(),
    );
}

#[test]
fn full_order_breaks_and_inactive_endpoint_controls_split_without_coupling_their_blocks() {
    for knots in [
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
        vec![-2., 0., 0., 0., 0.5, 1., 2., 3.],
    ] {
        let n = knots.len() - 3;
        let source = NurbsSurface::try_new_rational(
            2,
            2,
            n,
            n,
            (0..n * n)
                .map(|i| {
                    WeightedPoint3::try_new(
                        Point3::try_new((i % n) as f64, (i / n) as f64, ((i + i / n) % 3) as f64)
                            .unwrap(),
                        0.7 + (i % 4) as f64 / 4.,
                    )
                    .unwrap()
                })
                .collect(),
            knots.clone(),
            knots,
        )
        .unwrap();
        for source in [
            source.clone(),
            source.try_reversed_u().unwrap().try_reversed_v().unwrap(),
        ] {
            for direction in [SurfaceKnotDirection::U, SurfaceKnotDirection::V] {
                let p = if *source.domain_u().start() == 0. {
                    0.5
                } else {
                    -0.5
                };
                let actual = split_surface(&source, direction, p).unwrap();
                let expected = if direction == SurfaceKnotDirection::U {
                    source.try_split_u(p).unwrap()
                } else {
                    source.try_split_v(p).unwrap()
                };
                compare(&actual.0, &expected.0);
                compare(&actual.1, &expected.1);
            }
        }
    }
}

#[test]
fn unchanged_clamped_pole_controls_keep_their_exact_payload_after_native_projection() {
    let frame = Frame3::try_from_x_and_normal(
        Point3::try_new(5., -0.031, 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = NurbsSurface::try_sphere(frame, 0.8_f64.hypot(0.04)).unwrap();
    for (interval, old_row, new_row) in [
        (*source.domain_v().start()..=0.037, 0, 0),
        (
            0.037..=*source.domain_v().end(),
            source.control_point_count_v() - 1,
            usize::MAX,
        ),
    ] {
        let actual = trim_surface_direction(&source, SurfaceKnotDirection::V, interval).unwrap();
        let new_row = if new_row == usize::MAX {
            actual.control_point_count_v() - 1
        } else {
            new_row
        };
        for u in 0..source.control_point_count_u() {
            assert_eq!(
                actual.control_point(u, new_row),
                source.control_point(u, old_row)
            );
        }
    }
}

#[test]
fn mixed_affine_and_rational_lines_keep_exact_plane_crossings_noops_and_validation() {
    let source = NurbsSurface::try_new_rational(
        1,
        2,
        2,
        3,
        (0..3)
            .flat_map(|j| {
                [
                    (Point3::try_new(-1., -2. + j as f64, 7.).unwrap(), 1.),
                    (
                        Point3::try_new(2., 4. + j as f64, 7.).unwrap(),
                        if j == 1 { 2. } else { 1. },
                    ),
                ]
            })
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
        vec![1e9, 1e9, 1e9 + 3., 1e9 + 3.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let trimmed =
        trim_surface_direction(&source, SurfaceKnotDirection::U, 1e9 + 1.0..=1e9 + 2.0).unwrap();
    assert_eq!(
        trimmed.control_points()[0].point(),
        Point3::try_new(0., 0., 7.).unwrap()
    );
    assert_eq!(
        trimmed.control_points()[1].point(),
        Point3::try_new(1., 2., 7.).unwrap()
    );
    compare(
        &trimmed,
        &source.try_trimmed_u(1e9 + 1.0..=1e9 + 2.0).unwrap(),
    );
    assert_eq!(
        trim_surface(&source, source.domain_u(), source.domain_v()).unwrap(),
        source
    );
    for range in [f64::NAN..=1e9 + 1., 1e9 + 1.0..=1e9 + 1.0, 0.0..=1e9 + 1.] {
        assert!(matches!(
            trim_surface_direction(&source, SurfaceKnotDirection::U, range),
            Err(Error::Geometry(GeometryError::InvalidCurveTrimInterval))
        ));
    }
    for p in [f64::NAN, 1e9, 1e9 + 3.] {
        assert!(matches!(
            split_surface(&source, SurfaceKnotDirection::U, p),
            Err(Error::Geometry(_))
        ));
    }
    assert!(matches!(
        trim_surface_direction(&source, SurfaceKnotDirection::Both, source.domain_u()),
        Err(Error::Geometry(_))
    ));
}

#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
