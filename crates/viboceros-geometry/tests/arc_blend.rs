use serde_json::Value;
use viboceros_geometry::{ArcBlendOptions, Point3, Tolerance, Vector3, try_arc_blend};
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
#[test]
fn independently_constructed_biarcs_match_public_native_api_points_and_tangents() {
    use std::io::Read;
    let mut rows = Vec::new();
    for family in ["arc_blend_sdk", "arc_blend_unit"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../tools/rhino_oracle/observations/{family}.json.gz"
        ));
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap())
            .read_to_end(&mut bytes)
            .unwrap();
        let data: Value = serde_json::from_slice(&bytes).unwrap();
        rows.extend(data["results"].as_array().unwrap().clone());
    }
    let (mut matched, mut rejected, mut diagnostics) = (0, 0, 0);
    for row in &rows {
        let v = &row["value"];
        let c = &v["case"];
        let result = try_arc_blend(
            point(&c["start"]),
            Vector3::try_from(point(&c["start_direction"]).to_array()).unwrap(),
            point(&c["end"]),
            Vector3::try_from(point(&c["end_direction"]).to_array()).unwrap(),
            ArcBlendOptions {
                ratio: c["ratio"].as_f64().unwrap(),
                ..Default::default()
            },
            Tolerance::NUMERICAL_VALIDATION,
        );
        assert_eq!(
            result.is_ok(),
            v["success"] == true,
            "{}: {:?}",
            row["id"],
            result
        );
        let Ok(curve) = result else {
            rejected += 1;
            println!(
                "ARC_BLEND_REJECTION {}",
                serde_json::json!({"id":row["id"]})
            );
            continue;
        };
        let segments = curve.segments();
        let a = segments[0].as_ref();
        let b = segments[1].as_ref();
        let a = a.evaluate_with_tangent(*a.domain().end()).unwrap();
        let b = b.evaluate_with_tangent(*b.domain().start()).unwrap();
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-10);
        assert!(
            Vector3::try_from(a.tangent().as_vector().to_array())
                .unwrap()
                .dot(b.tangent().as_vector())
                .unwrap()
                > 1. - 1e-12
        );
        if row["id"].as_str().unwrap().starts_with("spatial_") {
            // Native CreateArcBlend assumes unit directions. The raw-vector
            // diagnostic has a visible tangent discontinuity at its junction.
            let a = point(&v["segments"][0]["samples"][32]["tangent"]);
            let b = point(&v["segments"][1]["samples"][0]["tangent"]);
            assert!(a.distance_to(b).unwrap() > 1e-3);
            diagnostics += 1;
            println!(
                "ARC_BLEND_DIAGNOSTIC {}",
                serde_json::json!({"id":row["id"],"native_join_tangent_difference":a.distance_to(b).unwrap()})
            );
            continue;
        }
        matched += 1;
        let curve = viboceros_geometry::CurveRef::PolyCurve(&curve);
        let nurbs = curve.to_nurbs().unwrap();
        let coefficient = coefficient_error(&nurbs, &v["curve"]);
        let domain = curve.domain();
        let mut error: f64 = coefficient.max(
            (v["curve"]["domain"][0].as_f64().unwrap() - *domain.start())
                .abs()
                .max((v["curve"]["domain"][1].as_f64().unwrap() - *domain.end()).abs()),
        );
        for sample in v["samples"].as_array().unwrap() {
            let t = sample["parameter"].as_f64().unwrap();
            let actual = curve
                .evaluate_with_tangent(t.clamp(*domain.start(), *domain.end()))
                .unwrap();
            error = error.max(actual.point().distance_to(point(&sample["point"])).unwrap());
            let expected = Vector3::try_from(point(&sample["tangent"]).to_array()).unwrap();
            let delta = Vector3::try_from(std::array::from_fn(|i| {
                actual.tangent().as_vector().to_array()[i] - expected.to_array()[i]
            }))
            .unwrap();
            error = error.max(delta.length().unwrap());
        }
        println!(
            "ARC_BLEND_SDK {}",
            serde_json::json!({"id":row["id"],"maximum_point_and_tangent_error":error})
        );
        assert!(error <= 1e-6, "{}: {error:e}", row["id"]);
    }
    assert_eq!((matched, rejected, diagnostics), (20, 17, 3));
}

fn coefficient_error(curve: &viboceros_geometry::NurbsCurve, expected: &Value) -> f64 {
    assert_eq!(
        curve.degree(),
        expected["degree"].as_u64().unwrap() as usize
    );
    let controls = expected["control_points"].as_array().unwrap();
    assert_eq!(curve.control_points().len(), controls.len());
    let mut error: f64 = 0.;
    for (a, b) in curve.control_points().iter().zip(controls) {
        error = error
            .max(a.point().distance_to(point(&b["point"])).unwrap())
            .max((a.weight() - b["weight"].as_f64().unwrap()).abs());
    }
    let knots: Vec<f64> = serde_json::from_value(expected["knots"].clone()).unwrap();
    assert_eq!(curve.knots().len(), knots.len());
    for (a, b) in curve.knots().iter().zip(knots) {
        error = error.max((a - b).abs());
    }
    error
}
#[test]
fn ordinary_and_alternate_biarcs_match_native_command_coefficients() {
    use std::io::Read;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/observations/arc_blend_command.json.gz");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    let data: Value = serde_json::from_slice(&bytes).unwrap();
    for row in data["results"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        if !["arc_default", "arc_s_shape", "arc_spatial", "arc_alternate"].contains(&id) {
            continue;
        }
        let v = &row["value"];
        let recipes = &v["prescribed_case"]["curves"];
        let a = &recipes[0];
        let b = &recipes[1];
        let start = point(&a["end"]);
        let end = point(&b["start"]);
        let curve = try_arc_blend(
            start,
            point(&a["start"]).vector_to(start).unwrap(),
            end,
            end.vector_to(point(&b["end"])).unwrap(),
            ArcBlendOptions {
                alternate: id == "arc_alternate",
                ..Default::default()
            },
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        let nurbs = curve.to_nurbs().unwrap();
        let error = coefficient_error(
            &nurbs,
            &v["after"].as_array().unwrap().last().unwrap()["curve"],
        );
        println!(
            "ARC_BLEND_COMMAND_GEOMETRY {}",
            serde_json::json!({"id":id,"maximum_coefficient_error":error})
        );
        assert!(error <= 1e-6, "{id}: {error:e}");
    }
}
