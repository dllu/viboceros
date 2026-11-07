use super::*;
use crate::{ArcBlendOptions, CurveRef, CurveSegment3, Tolerance, try_arc_blend};
use serde_json::Value;
use std::io::Read;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
#[test]
fn analytic_trial_radii_and_curves_match_public_sdk_samples() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/observations/arc_blend_radius_sdk.json.gz");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    let data: Value = serde_json::from_slice(&bytes).unwrap();
    let rows = data["results"].as_array().unwrap();
    assert_eq!(rows.len(), 48);
    for row in rows {
        let v = &row["value"];
        let c = &v["case"];
        assert_eq!(v["success"], true);
        let start = point(&c["start"]);
        let end = point(&c["end"]);
        let t0 = Vector3::try_from(point(&c["start_direction"]).to_array()).unwrap();
        let t1 = Vector3::try_from(point(&c["end_direction"]).to_array()).unwrap();
        let ratio = c["ratio"].as_f64().unwrap();
        let frame = Frame::new(start, t0, end, t1).unwrap();
        let curve = try_arc_blend(
            start,
            t0,
            end,
            t1,
            ArcBlendOptions {
                ratio,
                alternate: false,
            },
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        let diff = v["segments"][0]["arc"]["radius"].as_f64().unwrap()
            - v["segments"][1]["arc"]["radius"].as_f64().unwrap();
        let mut error = (frame
            .radius_difference(ratio / (1. + ratio), false)
            .unwrap()
            - diff)
            .abs();
        error = error.max(samples(CurveRef::PolyCurve(&curve), v));
        for (actual, expected) in curve
            .segments()
            .iter()
            .zip(v["segments"].as_array().unwrap())
        {
            let CurveSegment3::Arc(arc) = actual else {
                panic!("expected analytic arc")
            };
            error = error.max((arc.radius() - expected["arc"]["radius"].as_f64().unwrap()).abs());
            error = error.max(samples(CurveRef::Arc(arc), expected));
        }
        println!(
            "ARC_BLEND_RADIUS_SEARCH_SDK {}",
            serde_json::json!({"id":row["id"],"maximum_point_tangent_radius_and_coefficient_error":error})
        );
        assert!(error <= 1e-6, "{}: {error:e}", row["id"]);
    }
}
fn samples(actual: CurveRef<'_>, expected: &Value) -> f64 {
    let nurbs = actual.to_nurbs().unwrap();
    let c = &expected["curve"];
    let mut error = 0_f64;
    assert_eq!(nurbs.degree(), c["degree"].as_u64().unwrap() as usize);
    assert_eq!(
        nurbs.control_points().len(),
        c["control_points"].as_array().unwrap().len()
    );
    for (a, b) in nurbs
        .control_points()
        .iter()
        .zip(c["control_points"].as_array().unwrap())
    {
        error = error
            .max(a.point().distance_to(point(&b["point"])).unwrap())
            .max((a.weight() - b["weight"].as_f64().unwrap()).abs());
    }
    // Segment SDK domains belong to the outer composite; compare independent
    // local stations. The whole composite's knots retain its full domain.
    if matches!(actual, CurveRef::PolyCurve(_)) {
        assert_eq!(nurbs.knots().len(), c["knots"].as_array().unwrap().len());
        for (a, b) in nurbs.knots().iter().zip(c["knots"].as_array().unwrap()) {
            error = error.max((a - b.as_f64().unwrap()).abs());
        }
    }
    let domain = &c["domain"];
    let lo = domain[0].as_f64().unwrap();
    let hi = domain[1].as_f64().unwrap();
    assert_eq!(expected["samples"].as_array().unwrap().len(), 33);
    for s in expected["samples"].as_array().unwrap() {
        let a = ((s["parameter"].as_f64().unwrap() - lo) / (hi - lo)).clamp(0., 1.);
        let t = *actual.domain().start() + a * (*actual.domain().end() - *actual.domain().start());
        let jet = actual.evaluate_with_tangent(t).unwrap();
        error = error.max(jet.point().distance_to(point(&s["point"])).unwrap());
        for (x, y) in jet
            .tangent()
            .as_vector()
            .to_array()
            .iter()
            .zip(s["tangent"].as_array().unwrap())
        {
            error = error.max((x - y.as_f64().unwrap()).abs());
        }
    }
    error
}
