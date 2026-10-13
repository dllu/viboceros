#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    CircularArc3, Curve3, CurveBlendContinuity, CurveMatchPreserveEnd, LineSegment, NurbsCurve,
    Point3, Tolerance, try_average_match_curve_ends, try_match_curve_end,
};
use viboceros_smlib::{average_match_curve_ends, match_curve_end};
#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn curve(v: &Value) -> Curve3 {
    match v["type"].as_str().unwrap() {
        "line" => Curve3::Line(
            LineSegment::try_new(point(&v["start"]), point(&v["end"]), Tolerance::DEFAULT).unwrap(),
        ),
        "arc" => Curve3::Arc(
            CircularArc3::try_from_three_points(
                point(&v["points"][0]),
                point(&v["points"][1]),
                point(&v["points"][2]),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        "nurbs" => Curve3::NurbsCurve(
            NurbsCurve::try_new_rational(
                v["degree"].as_u64().unwrap() as usize,
                support::points(&v["control_points"]),
                support::knots(&v["knots"]),
            )
            .unwrap(),
        ),
        _ => unreachable!(),
    }
}
fn compare(a: &NurbsCurve, b: &NurbsCurve) {
    assert_eq!(a.degree(), b.degree());
    assert_eq!(a.knots(), b.knots());
    assert_eq!(a.control_points().len(), b.control_points().len());
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert!(
            (a.weight() - b.weight()).abs()
                < 1e-10 + 1e-12 * a.weight().abs().max(b.weight().abs())
        );
        for (a, b) in a.point().to_array().into_iter().zip(b.point().to_array()) {
            assert!((a - b).abs() < 1e-9 + 1e-12 * a.abs().max(b.abs()));
        }
    }
    for i in 0..=64 {
        let t = a.parameter_at(i as f64 / 64.).unwrap();
        assert!(
            a.evaluate(t)
                .unwrap()
                .distance_to(b.evaluate(t).unwrap())
                .unwrap()
                < 1e-8
        );
    }
}
#[test]
fn native_match_preparation_preserves_continuity_preservation_averaging_and_rational_cases() {
    let mut cases = 0;
    for f in [
        include_str!("../../../tools/rhino_oracle/fixtures/curve_match_geometry.json"),
        include_str!("../../../tools/rhino_oracle/fixtures/curve_match_average_curvature.json"),
        include_str!("../../../tools/rhino_oracle/fixtures/curve_match_multispan_position.json"),
        include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_match_five_control_rational_translation_sweep.json"
        ),
    ] {
        let d: Value = serde_json::from_str(f).unwrap();
        let tolerance = d.get("tolerance").map_or(Tolerance::DEFAULT, |t| {
            Tolerance::try_new(
                t["absolute"].as_f64().unwrap(),
                t["relative"].as_f64().unwrap(),
                t["angular"].as_f64().unwrap(),
            )
            .unwrap()
        });
        for op in d["operations"].as_array().unwrap() {
            let a = curve(&op["first"]);
            let b = curve(&op["second"]);
            let before = (a.clone(), b.clone());
            let ends = [
                op["reverse_first"].as_bool().unwrap_or(false),
                op["reverse_second"].as_bool().unwrap_or(false),
            ];
            let continuity = match op["continuity"].as_str().unwrap() {
                "Position" => CurveBlendContinuity::Position,
                "Tangency" => CurveBlendContinuity::Tangency,
                "Curvature" => CurveBlendContinuity::Curvature,
                _ => unreachable!(),
            };
            let preserve = match op["preserve_other_end"].as_str().unwrap_or("None") {
                "None" => CurveMatchPreserveEnd::None,
                "Position" => CurveMatchPreserveEnd::Position,
                "Tangency" => CurveMatchPreserveEnd::Tangency,
                "Curvature" => CurveMatchPreserveEnd::Curvature,
                _ => unreachable!(),
            };
            if op["average"].as_bool().unwrap_or(false) {
                let actual = average_match_curve_ends(
                    (&a, ends[0]),
                    (&b, ends[1]),
                    continuity,
                    preserve,
                    tolerance,
                )
                .unwrap();
                let expected = try_average_match_curve_ends(
                    &a, ends[0], &b, ends[1], continuity, preserve, tolerance,
                )
                .unwrap();
                compare(&actual.0, &expected.0);
                compare(&actual.1, &expected.1);
            } else {
                let actual = match_curve_end(
                    (&a, ends[0]),
                    (&b, ends[1]),
                    continuity,
                    preserve,
                    tolerance,
                )
                .unwrap();
                let expected =
                    try_match_curve_end(&a, ends[0], &b, ends[1], continuity, preserve, tolerance)
                        .unwrap();
                compare(&actual, &expected);
            }
            assert_eq!((a, b), before);
            cases += 1;
        }
    }
    assert_eq!(cases, 59);
}

#[test]
fn affine_preparation_rejects_oversized_public_requests_before_solving() {
    let source = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    assert!(matches!(
        viboceros_smlib::interpolate_affine_greville(&source, 1001),
        Err(viboceros_smlib::Error::Geometry(
            viboceros_geometry::GeometryError::InvalidCurveRebuildPointCount { .. }
        ))
    ));
    assert_eq!(source, before);
}
