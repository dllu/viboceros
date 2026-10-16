use super::*;
use serde_json::Value;
use viboceros_geometry::{Circle3, Curve3, LineSegment, Polyline3, Vector3};
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn curve(v: &Value) -> Curve3 {
    match v["type"].as_str().unwrap() {
        "line" => Curve3::Line(
            LineSegment::try_new(
                point(&v["start"]),
                point(&v["end"]),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap(),
        ),
        "circle" => Curve3::Circle(
            Circle3::try_new(
                point(&v["center"]),
                v["radius"].as_f64().unwrap(),
                Vector3::try_new(0., 0., 1.)
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        "polyline" => Curve3::Polyline(
            Polyline3::try_new(
                v["vertices"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(point)
                    .collect(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        "nurbs" => Curve3::NurbsCurve(
            NurbsCurve::try_new(
                v["degree"].as_u64().unwrap() as usize,
                v["control_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| point(&v["point"]))
                    .collect(),
                serde_json::from_value(v["knots"].clone()).unwrap(),
            )
            .unwrap(),
        ),
        other => panic!("{other}"),
    }
}
#[test]
fn curve_distance_corpus_retains_native_overlap_and_minimum_disagreements() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/curve_deviation.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/curve_deviation_rhino_20261016.json"
    ))
    .unwrap();
    let (mut scalar_matches, mut minimum_gaps, mut overlap_gaps, mut empty) = (0, 0, 0, 0);
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        let a = curve(&op["curves"][0]);
        let b = curve(&op["curves"][1]);
        let refs = [a.as_ref(), b.as_ref()];
        let intervals = projected_curve_intervals(refs[0], refs[1], Tolerance::DEFAULT).unwrap();
        let fit = intervals
            .and_then(|i| curve_distance_extrema(refs[0], refs[1], i, Tolerance::DEFAULT).unwrap());
        let sdk = &row["value"]["sdk"];
        if let Some(f) = fit {
            assert!(f.maximum.distance >= f.minimum.distance);
            for w in [&f.minimum, &f.maximum] {
                for (i, c) in refs.iter().enumerate() {
                    assert!(
                        c.evaluate(w.parameters[i])
                            .unwrap()
                            .distance_bound_to(w.points[i], 1e-6)
                            .unwrap()
                            .is_some()
                    );
                }
                assert_eq!(w.distance, w.points[0].distance_to(w.points[1]).unwrap());
            }
            if !sdk["created"].as_bool().unwrap() {
                assert!(matches!(
                    op["id"].as_str().unwrap(),
                    "crossing-circles" | "separated-circles"
                ));
                overlap_gaps += 1;
                continue;
            }
            let max_error =
                (f.maximum.distance - sdk["maximum"]["distance"].as_f64().unwrap()).abs();
            assert!(max_error <= 1e-6, "{} max {max_error}", op["id"]);
            let min_error =
                (f.minimum.distance - sdk["minimum"]["distance"].as_f64().unwrap()).abs();
            if matches!(
                op["id"].as_str().unwrap(),
                "curved-offset" | "curved-spatial"
            ) {
                assert!(min_error > 1e-4);
                assert!(f.minimum.distance < sdk["minimum"]["distance"].as_f64().unwrap() - 1e-4);
                minimum_gaps += 1;
            } else {
                assert!(min_error <= 1e-6, "{} min {min_error}", op["id"]);
                scalar_matches += 1;
            }
        } else {
            assert!(!sdk["created"].as_bool().unwrap());
            empty += 1;
        }
    }
    assert_eq!(
        (scalar_matches, minimum_gaps, overlap_gaps, empty),
        (11, 2, 2, 3)
    );
}
#[test]
fn curve_distance_explicit_intervals_and_reversal_preserve_line_extrema() {
    for reverse in [false, true] {
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let a = LineSegment::try_new(
            p(if reverse { 10. } else { 0. }, 0.),
            p(if reverse { 0. } else { 10. }, 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let b = LineSegment::try_new(p(3., 2.), p(7., 2.), Tolerance::DEFAULT).unwrap();
        let report = curve_distance_extrema(
            CurveRef::Line(&a),
            CurveRef::Line(&b),
            [[0.3, 0.7], [0., 1.]],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .unwrap();
        assert_eq!(report.minimum.distance, 2.);
        assert_eq!(report.maximum.distance, 2.);
        assert!(
            curve_distance_extrema(
                CurveRef::Line(&a),
                CurveRef::Line(&b),
                [[0.3, 0.3], [0., 1.]],
                Tolerance::DEFAULT
            )
            .unwrap()
            .is_none()
        );
        assert!(
            curve_distance_extrema(
                CurveRef::Line(&a),
                CurveRef::Line(&b),
                [[0.3, 0.3], [f64::NAN, 1.]],
                Tolerance::DEFAULT
            )
            .is_err()
        );
        assert!(
            curve_distance_extrema(
                CurveRef::Line(&a),
                CurveRef::Line(&b),
                [[-0.1, 1.], [0., 1.]],
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
}
#[test]
fn curve_distance_rational_arc_translation_keeps_constant_normal_gap() {
    use viboceros_geometry::{AffineTransform3, CircularArc3};
    let arc = CircularArc3::try_from_three_points(
        Point3::try_new(2., 0., 0.).unwrap(),
        Point3::try_new(0., 2., 0.).unwrap(),
        Point3::try_new(-2., 0., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let a = CurveRef::Arc(&arc).to_nurbs().unwrap();
    let b = a
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(0., 0., 0.5).unwrap(),
        ))
        .unwrap();
    let r = curve_distance_extrema(
        CurveRef::NurbsCurve(&a),
        CurveRef::NurbsCurve(&b),
        [[0., 1.]; 2],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .unwrap();
    assert!((r.minimum.distance - 0.5).abs() < 1e-12);
    assert!((r.maximum.distance - 0.5).abs() < 1e-12);
}
#[test]
fn curve_distance_localized_peak_is_sampled_inside_every_nonempty_span() {
    let delta = 1. / 4096.;
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let a = NurbsCurve::try_new(
        2,
        vec![
            p(0., 0.),
            p(2.5, 0.),
            p(5., 0.),
            p(5. + 5. * delta, 2.),
            p(5. + 10. * delta, 0.),
            p(7.5, 0.),
            p(10., 0.),
        ],
        vec![0., 0., 0., 0.5, 0.5, 0.5 + delta, 0.5 + delta, 1., 1., 1.],
    )
    .unwrap();
    let b = LineSegment::try_new(p(0., 0.), p(10., 0.), Tolerance::DEFAULT).unwrap();
    let r = curve_distance_extrema(
        CurveRef::NurbsCurve(&a),
        CurveRef::Line(&b),
        [[0., 1.]; 2],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .unwrap();
    assert!((r.maximum.distance - 1.).abs() < 1e-8);
    assert!(r.minimum.distance < 1e-8);
}
#[test]
fn curve_distance_closed_polyline_portion_preserves_the_requested_parameter_visit() {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let a = Polyline3::try_new(
        vec![p(0., 0.), p(1., 1.), p(0., 1.), p(1., 0.), p(0., 0.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let b = LineSegment::try_new(p(0., -0.2), p(1., -0.2), Tolerance::DEFAULT).unwrap();
    let r = curve_distance_extrema(
        CurveRef::Polyline(&a),
        CurveRef::Line(&b),
        [[0.75, 1.], [0., 1.]],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .unwrap();
    for w in [r.minimum, r.maximum] {
        assert!((w.distance - 0.2).abs() < 1e-12);
        assert!((3. ..=4.).contains(&w.parameters[0]));
        assert!((0.75..=1.).contains(&w.normalized_parameters[0]));
    }
}
