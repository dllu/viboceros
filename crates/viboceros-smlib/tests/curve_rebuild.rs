#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    Circle3, CurveRef, GeometryError, NurbsCurve, Point3, Polyline3, Tolerance, UnitVector3,
    try_rebuild_curve,
};
use viboceros_smlib::{Error, rebuild_curve};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn compare(a: &NurbsCurve, e: &NurbsCurve) {
    support::compare(
        &support::curve_value(a),
        &support::curve_value(e),
        "rebuild",
    );
    for i in 0..=64 {
        let t = a.parameter_at(i as f64 / 64.).unwrap();
        for (a, e) in a
            .evaluate(t)
            .unwrap()
            .to_array()
            .into_iter()
            .zip(e.evaluate(t).unwrap().to_array())
        {
            assert!(
                (a - e).abs() <= 1e-10 + 1e-12 * a.abs().max(e.abs()),
                "{a} != {e}"
            );
        }
    }
}

#[test]
fn native_rebuild_preserves_permanent_inputs_controls_knots_and_tangent_projection() {
    let f: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_rebuild.json"
    ))
    .unwrap();
    for op in f["operations"].as_array().unwrap() {
        let c = &op["curve"];
        let source = NurbsCurve::try_new_rational(
            c["degree"].as_u64().unwrap() as usize,
            support::points(&c["control_points"]),
            support::knots(&c["knots"]),
        )
        .unwrap();
        let before = source.clone();
        let count = op["point_count"].as_u64().unwrap() as usize;
        let degree = op["degree"].as_u64().unwrap() as usize;
        let tangents = op["preserve_tangents"].as_bool().unwrap_or(false);
        compare(
            &rebuild_curve(
                CurveRef::NurbsCurve(&source),
                count,
                degree,
                tangents,
                Tolerance::DEFAULT,
            )
            .unwrap(),
            &try_rebuild_curve(
                CurveRef::NurbsCurve(&source),
                count,
                degree,
                tangents,
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        assert_eq!(source, before);
    }
}

#[test]
fn native_rebuild_covers_all_degrees_even_odd_periodic_counts_and_resource_boundaries() {
    let open = Polyline3::try_new(
        vec![
            point(0., 0., 0.),
            point(2., 3., 1.),
            point(6., -1., -2.),
            point(8., 1., 0.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let closed = Polyline3::try_new(
        vec![
            point(0., 0., 0.),
            point(2., 3., 1.),
            point(6., -1., -2.),
            point(8., 1., 0.),
            point(0., 0., 0.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for degree in 1..=11 {
        for count in [1, 3, 8, 31, 32, 1000] {
            for source in [&open, &closed] {
                for tangents in [false, true] {
                    compare(
                        &rebuild_curve(
                            CurveRef::Polyline(source),
                            count,
                            degree,
                            tangents,
                            Tolerance::DEFAULT,
                        )
                        .unwrap(),
                        &try_rebuild_curve(
                            CurveRef::Polyline(source),
                            count,
                            degree,
                            tangents,
                            Tolerance::DEFAULT,
                        )
                        .unwrap(),
                    );
                }
            }
        }
    }
}

#[test]
fn periodic_native_collocation_keeps_analytic_circle_greville_stations() {
    let circle = Circle3::try_new(
        point(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for degree in 1..=11 {
        for count in [degree.max(3), 31, 32, 1000] {
            let curve = rebuild_curve(
                CurveRef::Circle(&circle),
                count,
                degree,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(curve.is_periodic(), degree > 1);
            assert!(curve.control_points().iter().all(|p| p.weight() == 1.));
            let offset = if degree % 2 == 0 { 0.5 } else { 0. };
            for i in 0..count {
                let t = i as f64 + offset;
                let theta = std::f64::consts::TAU * t / count as f64;
                let p = curve.evaluate(t).unwrap();
                assert!((p.x() - 3. * theta.cos()).abs() < 1e-10);
                assert!((p.y() - 3. * theta.sin()).abs() < 1e-10);
                assert_eq!(p.z(), 0.);
            }
        }
    }
}

#[test]
fn native_rebuild_retains_small_channels_and_typed_option_errors() {
    for tiny in [1e-18, 1e-180, 1e-280] {
        let source = Polyline3::try_new(
            vec![
                point(0., 0., 0.),
                point(1., 0., tiny),
                point(2., 0., -tiny),
                point(0., 0., 0.),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        for degree in [2, 3, 11] {
            let a = rebuild_curve(
                CurveRef::Polyline(&source),
                32,
                degree,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
            let e = try_rebuild_curve(
                CurveRef::Polyline(&source),
                32,
                degree,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
            for (a, e) in a.control_points().iter().zip(e.control_points()) {
                assert!((a.point().z() / tiny - e.point().z() / tiny).abs() < 1e-10);
            }
        }
    }
    let source = Polyline3::try_new(
        vec![point(0., 0., 0.), point(1., 0., 0.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(matches!(
        rebuild_curve(
            CurveRef::Polyline(&source),
            1001,
            3,
            false,
            Tolerance::DEFAULT
        ),
        Err(Error::Geometry(
            GeometryError::InvalidCurveRebuildPointCount { .. }
        ))
    ));
    assert!(matches!(
        rebuild_curve(
            CurveRef::Polyline(&source),
            10,
            12,
            false,
            Tolerance::DEFAULT
        ),
        Err(Error::Geometry(
            GeometryError::InvalidCurveRebuildDegree { .. }
        ))
    ));
}

#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
