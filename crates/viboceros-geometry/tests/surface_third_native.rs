//! Evaluate prescribed surfaces against passive public Rhino surface partials.
use serde_json::Value;
use std::io::Read;
use viboceros_geometry::{NurbsSurface, Point3, WeightedPoint3};
#[path = "../../viboceros-command/tests/support/blend_curve_surface_high.rs"]
#[allow(dead_code)]
mod families;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree_u"].as_u64().unwrap() as usize,
        v["degree_v"].as_u64().unwrap() as usize,
        v["control_point_count_u"].as_u64().unwrap() as usize,
        v["control_point_count_v"].as_u64().unwrap() as usize,
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
#[test]
fn analytic_third_partials_match_public_native_surface_evaluation() {
    let mut workflows = 0;
    for family in families::NEW_FAMILIES {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../tools/rhino_oracle/observations/{family}.json.gz"
        ));
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap())
            .read_to_end(&mut bytes)
            .unwrap();
        let data: Value = serde_json::from_slice(&bytes).unwrap();
        for row in data["results"].as_array().unwrap() {
            let value = &row["value"];
            let mut stations = 0;
            let mut points = 0_f64;
            let mut relative = 0_f64;
            let mut absolute = 0_f64;
            for click in value["input_trace"][0]["clicks"].as_array().unwrap() {
                let Some(context) = click.get("edge_pick_context") else {
                    continue;
                };
                let action = click["action"].as_u64().unwrap() as usize;
                let source = value["prescribed_case"]["actions"][action]["curve"]
                    .as_u64()
                    .unwrap() as usize;
                let surface = surface(&value["prescribed_case"]["curves"][source]);
                for candidate in context["candidates"].as_array().unwrap() {
                    for face in candidate["faces"].as_array().unwrap() {
                        assert_eq!(face["face"], 0);
                        assert_eq!(face["third_order"]["success"], true);
                        let uv = &face["uv"];
                        let jet = surface
                            .evaluate_with_third_derivatives(
                                uv[0].as_f64().unwrap(),
                                uv[1].as_f64().unwrap(),
                            )
                            .unwrap();
                        points = points.max(
                            jet.second
                                .point
                                .distance_to(point(&face["third_order"]["point"]))
                                .unwrap(),
                        );
                        let expected = face["third_order"]["partials"].as_array().unwrap();
                        assert_eq!(expected.len(), 9);
                        for (actual, expected) in jet.partials().into_iter().zip(expected) {
                            let expected: [f64; 3] =
                                serde_json::from_value(expected.clone()).unwrap();
                            for (a, b) in actual.to_array().into_iter().zip(expected) {
                                let e = (a - b).abs();
                                absolute = absolute.max(e);
                                relative = relative.max(e / a.abs().max(b.abs()).max(1.));
                            }
                        }
                        stations += 1;
                    }
                }
            }
            assert!(stations > 0);
            assert!(
                points <= 1e-6 && relative <= 1e-8,
                "{family}/{}: {points:e}, {relative:e}",
                row["id"]
            );
            println!(
                "SURFACE_THIRD_NATIVE {}",
                serde_json::json!({"id":format!("{family}/{}",row["id"].as_str().unwrap()),"stations":stations,"maximum_point_error":points,"maximum_partial_relative_error":relative,"maximum_partial_absolute_error":absolute})
            );
            workflows += 1;
        }
    }
    assert!(workflows >= 20);
}
