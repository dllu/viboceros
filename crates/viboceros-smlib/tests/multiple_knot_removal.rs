#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    GeometryError, NurbsCurve, NurbsSurface, Point3, SurfaceKnotDirection, WeightedPoint3,
};
use viboceros_smlib::{remove_curve_multiple_knots, remove_surface_multiple_knots};
#[path = "support/nurbs.rs"]
mod support;
use support::{compare, curve_value, knots, points, surface_value};

fn fixtures() -> (Value, Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/remove_multi_knot.json")).unwrap(),
    )
    .unwrap();
    let observation = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/remove_multi_knot.json")).unwrap(),
    )
    .unwrap();
    (request, observation)
}

fn curve_from(value: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        points(&value["control_points"]),
        knots(&value["knots"]),
    )
    .unwrap()
}

fn surface_from(value: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        value["degree_u"].as_u64().unwrap() as usize,
        value["degree_v"].as_u64().unwrap() as usize,
        value["control_point_count_u"].as_u64().unwrap() as usize,
        value["control_point_count_v"].as_u64().unwrap() as usize,
        points(&value["control_points"]),
        knots(&value["knots_u"]),
        knots(&value["knots_v"]),
    )
    .unwrap()
}

#[test]
fn native_multiple_removal_matches_every_raw_rhino_command_and_portable_count() {
    let (fixture, reference) = fixtures();
    assert_eq!(fixture["operations"].as_array().unwrap().len(), 11);
    assert_eq!(reference["results"].as_array().unwrap().len(), 11);
    for (op, record) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], record["id"]);
        let fully = op["remove_fully_multiple_knots"].as_bool().unwrap();
        let angle = op["maximum_kink_angle_degrees"]
            .as_f64()
            .unwrap()
            .to_radians();
        let actual = if let Some(c) = op.get("curve") {
            let source = curve_from(c);
            let before = source.clone();
            let (result, removed) = remove_curve_multiple_knots(&source, fully, angle).unwrap();
            let (_, expected_count) = source.try_remove_multiple_knots(fully, angle).unwrap();
            assert_eq!(removed, expected_count);
            assert_eq!(source, before);
            if removed == 0 {
                assert_eq!(result, source);
            }
            curve_value(&result)
        } else {
            let source = surface_from(op);
            let before = source.clone();
            let (result, removed) =
                remove_surface_multiple_knots(&source, SurfaceKnotDirection::Both, fully, angle)
                    .unwrap();
            let (_, expected_count) = source
                .try_remove_multiple_knots(SurfaceKnotDirection::Both, fully, angle)
                .unwrap();
            assert_eq!(removed, expected_count);
            assert_eq!(source, before);
            if removed == 0 {
                assert_eq!(result, source);
            }
            surface_value(&result)
        };
        compare(&actual, &record["value"], op["id"].as_str().unwrap());
    }
}

#[test]
fn curve_group_order_preserves_extreme_charts_gauges_noops_and_complete_controls() {
    let (fixture, _) = fixtures();
    let base = curve_from(&fixture["operations"][4]["curve"]);
    for scale in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            let source = NurbsCurve::try_new_rational(
                base.degree(),
                base.control_points()
                    .iter()
                    .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                    .collect(),
                base.knots().iter().map(|k| k * scale).collect(),
            )
            .unwrap();
            let before = source.clone();
            let (actual, count) =
                remove_curve_multiple_knots(&source, true, std::f64::consts::PI).unwrap();
            let (expected, expected_count) = source
                .try_remove_multiple_knots(true, std::f64::consts::PI)
                .unwrap();
            assert_eq!(count, expected_count);
            compare(
                &curve_value(&actual),
                &curve_value(&expected),
                "descending group refits",
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
            assert_eq!(
                remove_curve_multiple_knots(&source, true, 0.).unwrap(),
                (source.clone(), 0)
            );
            assert_eq!(source, before);
        }
    }
}

#[test]
fn tensor_direction_order_matches_portable_grids_and_preserves_unedited_bases() {
    let (fixture, _) = fixtures();
    let source = surface_from(&fixture["operations"][10]);
    let before = source.clone();
    for direction in [
        SurfaceKnotDirection::U,
        SurfaceKnotDirection::V,
        SurfaceKnotDirection::Both,
    ] {
        let (actual, count) = remove_surface_multiple_knots(&source, direction, false, 0.).unwrap();
        let (expected, expected_count) = source
            .try_remove_multiple_knots(direction, false, 0.)
            .unwrap();
        assert_eq!(count, expected_count);
        compare(
            &surface_value(&actual),
            &surface_value(&expected),
            "tensor group order",
        );
        if direction == SurfaceKnotDirection::U {
            assert_eq!(actual.knots_v(), source.knots_v());
        }
        if direction == SurfaceKnotDirection::V {
            assert_eq!(actual.knots_u(), source.knots_u());
        }
        for i in 0..=16 {
            for j in 0..=16 {
                let u = source.parameter_at_u(i as f64 / 16.).unwrap();
                let v = source.parameter_at_v(j as f64 / 16.).unwrap();
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
        assert_eq!(source, before);
    }
}

#[test]
fn rejection_and_empty_candidates_preserve_inputs_before_kernel_work() {
    let (fixture, _) = fixtures();
    let source = curve_from(&fixture["operations"][0]["curve"]);
    let before = source.clone();
    for angle in [f64::NAN, -1., 4.] {
        assert!(matches!(
            remove_curve_multiple_knots(&source, false, angle),
            Err(viboceros_smlib::Error::Geometry(
                GeometryError::InvalidKnotRemovalAngle
            ))
        ));
        assert_eq!(source, before);
    }
    let periodic = NurbsCurve::try_new(
        2,
        [
            [0., 0., 0.],
            [2., 0., 1.],
            [1., 2., 0.],
            [0., 0., 0.],
            [2., 0., 1.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![-2., -1., 0., 1., 2., 3., 4., 5.],
    )
    .unwrap();
    assert!(periodic.is_periodic());
    assert!(matches!(
        remove_curve_multiple_knots(&periodic, false, 1.),
        Err(viboceros_smlib::Error::Geometry(
            GeometryError::PeriodicKnotRemovalUnsupported { .. }
        ))
    ));
    let linear = curve_from(&fixture["operations"][3]["curve"]);
    assert_eq!(
        remove_curve_multiple_knots(&linear, false, 1.).unwrap(),
        (linear, 0)
    );
}

#[test]
fn both_direction_periodic_validation_precedes_unsupported_native_u_work() {
    let degree_u = 18;
    let count_u = degree_u + 3;
    let v_controls = [
        [0., 0., 0.],
        [2., 0., 1.],
        [1., 2., 0.],
        [0., 0., 0.],
        [2., 0., 1.],
    ];
    let mut u_knots = vec![0.; degree_u + 1];
    u_knots.extend([0.5, 0.5]);
    u_knots.extend(vec![1.; degree_u + 1]);
    let source = NurbsSurface::try_new(
        degree_u,
        2,
        count_u,
        5,
        v_controls
            .into_iter()
            .flat_map(|p| {
                (0..count_u).map(move |u| Point3::try_new(p[0] + u as f64, p[1], p[2]).unwrap())
            })
            .collect(),
        u_knots,
        vec![-2., -1., 0., 1., 2., 3., 4., 5.],
    )
    .unwrap();
    assert!(!source.is_periodic_u());
    assert!(source.is_periodic_v());
    let before = source.clone();
    assert!(matches!(
        remove_surface_multiple_knots(&source, SurfaceKnotDirection::Both, false, 1.),
        Err(viboceros_smlib::Error::Geometry(
            GeometryError::PeriodicKnotRemovalUnsupported {
                direction: "surface V direction"
            }
        ))
    ));
    assert!(matches!(
        remove_surface_multiple_knots(&source, SurfaceKnotDirection::U, false, 1.),
        Err(viboceros_smlib::Error::Kernel(_))
    ));
    assert_eq!(source, before);
}
