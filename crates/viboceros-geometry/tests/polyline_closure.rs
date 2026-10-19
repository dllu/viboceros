#[path = "support/polyline_closure.rs"]
mod reference;
use viboceros_geometry::{
    Curve3, CurveClosure, CurveRef, CurveSegment3, Point3, Polyline3, Tolerance,
};
#[test]
fn native_closure_uses_representation_specific_endpoint_and_interior_rules() {
    let data = reference::capture();
    let rows = data["results"][0]["value"]["rows"].as_array().unwrap();
    let mut checked = 0;
    for row in rows {
        if row["geometry"].is_null() {
            continue;
        }
        let name = row["recipe"]["id"].as_str().unwrap();
        let curve = reference::curve(&row["geometry"]);
        let original = curve.clone();
        assert_eq!(
            curve.as_ref().is_closed().unwrap(),
            row["closed"].as_bool().unwrap(),
            "{name}"
        );
        if let Curve3::Polyline(p) = &curve {
            assert_eq!(p.is_closed(), row["closed"].as_bool().unwrap(), "{name}");
        }
        if let Some(extract) = row.get("extract") {
            let points = match &curve {
                Curve3::PolyCurve(c) => c.extract_point_locations().unwrap(),
                _ => CurveSegment3::try_from_curve(&curve)
                    .unwrap()
                    .extract_point_locations()
                    .unwrap(),
            };
            let expected = serde_json::Value::Array(
                extract["value"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p["point"].clone())
                    .collect(),
            );
            assert!(
                reference::compare_points(&points, &expected, name, false) <= 1e-8,
                "{name}: extracted points"
            );
        }
        assert_eq!(curve, original, "{name}: immutable source");
        checked += 1;
    }
    assert!(checked > 100);
}
#[test]
fn fuzzy_closure_preserves_vertices_parameters_and_noop_closure_even_at_large_origins() {
    for origin in [0., 1e12] {
        for gap in [1e-10, 0.4] {
            if origin == 0. && gap > 1e-9 {
                continue;
            }
            let points = [
                [origin, 0., 0.],
                [origin + 4., 0., 0.],
                [origin + 4., 3., 0.],
                [origin + gap, 0., 0.],
            ]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec();
            let p = Polyline3::try_with_parameters(
                points,
                vec![-3., 2., 7., 17.],
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
            assert!(p.is_closed());
            let c = Curve3::Polyline(p.clone());
            let (result, outcome) = c.close(0., true, Tolerance::DEFAULT).unwrap();
            assert_eq!(outcome, CurveClosure::AlreadyClosed);
            assert_eq!(result, c);
            assert_eq!(
                CurveRef::Polyline(&p).start_point().unwrap(),
                p.vertices()[0]
            );
            assert_eq!(CurveRef::Polyline(&p).end_point().unwrap(), p.vertices()[3]);
            assert_eq!(
                CurveRef::Polyline(&p)
                    .divide_by_count(4, true, Tolerance::DEFAULT)
                    .unwrap()
                    .len(),
                4
            );
        }
    }
}
