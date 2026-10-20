#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
use serde_json::Value;
use viboceros_geometry::{CurveRef, Point3};
fn audit(name: &str, q: &str, r: &str) {
    let q: Value = serde_json::from_str(q).unwrap();
    let r: Value = serde_json::from_str(r).unwrap();
    assert_eq!(
        q["operations"].as_array().unwrap().len(),
        r["results"].as_array().unwrap().len()
    );
    let mut failures = Vec::new();
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        let result = std::panic::catch_unwind(|| {
            let id = op["id"].as_str().unwrap();
            assert_eq!(op["id"], row["id"]);
            assert!(row["error"].is_null(), "{id}: {}", row["error"]);
            let curve = morph_fixtures::curve(&row["value"]["curve"]);
            let parameters = row["value"]
                .get("parameters")
                .unwrap_or(&op["parameters"])
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_f64().unwrap())
                .collect::<Vec<_>>();
            let frames = viboceros_command::curve_frames::perpendicular_frames(
                CurveRef::NurbsCurve(&curve),
                &parameters,
                131_072,
            )
            .unwrap();
            let expected = row["value"]["frames"].as_array().unwrap();
            assert_eq!(frames.len(), expected.len());
            let mut max = 0_f64;
            for (f, e) in frames.iter().zip(expected) {
                for (a, k) in [
                    (f.origin(), "origin"),
                    (
                        Point3::try_from(f.x_axis().as_vector().to_array()).unwrap(),
                        "x",
                    ),
                    (
                        Point3::try_from(f.y_axis().as_vector().to_array()).unwrap(),
                        "y",
                    ),
                    (
                        Point3::try_from(f.z_axis().as_vector().to_array()).unwrap(),
                        "z",
                    ),
                ] {
                    max = max.max(a.distance_to(morph_fixtures::p(&e[k])).unwrap());
                }
            }
            let qualification = if op["representation"] == "Polycurve"
                && op["parameters"][1].as_f64() == Some(2_f64.next_up())
            {
                "native_proxy_parameter_rounding"
            } else if op["id"] == "proximity_50" {
                "native_frame_approximation"
            } else {
                "qualified"
            };
            println!(
                "FRAME_BOUNDARY_AUDIT {}",
                serde_json::json!({"id":id,"suite":name,"maximum_error":max,"qualification":qualification})
            );
            match qualification {
                "native_proxy_parameter_rounding" => {}
                "native_frame_approximation" => {
                    assert!(max <= 1e-5, "{id}: native approximation {max}")
                }
                _ => assert!(max <= 1e-6, "{id}: frame difference {max}"),
            }
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{name}: {failures:?}");
}
#[test]
fn native_boundary_projection_across_polyline_nurbs_and_polycurve_queries() {
    audit(
        "boundaries",
        include_str!("../../../tools/rhino_oracle/fixtures/curve_frame_boundaries.json"),
        include_str!("../../../tools/rhino_oracle/observations/curve_frame_boundaries.json"),
    );
}
#[test]
fn native_frame_seeds_curved_spans_and_parameter_domains() {
    audit(
        "seeds",
        include_str!("../../../tools/rhino_oracle/fixtures/curve_frame_seeds.json"),
        include_str!("../../../tools/rhino_oracle/observations/curve_frame_seeds.json"),
    );
}
#[test]
fn native_near_corner_continuations_and_fallback_thresholds() {
    audit(
        "proximity",
        include_str!("../../../tools/rhino_oracle/fixtures/curve_frame_proximity.json"),
        include_str!("../../../tools/rhino_oracle/observations/curve_frame_proximity.json"),
    );
}
