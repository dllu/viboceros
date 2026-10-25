//! Certify complete native fitted images, independently of their clocks.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_geometry::{NurbsCurve, Point3, Tolerance, WeightedPoint3};
use viboceros_oracle::{ProbeRequest, run_request};

fn curve(value: &Value) -> NurbsCurve {
    let controls = value["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let xyz: [f64; 3] = serde_json::from_value(c["point"].clone()).unwrap();
            WeightedPoint3::try_new(
                Point3::try_from(xyz).unwrap(),
                c["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        controls,
        serde_json::from_value(value["knots"].clone()).unwrap(),
    )
    .unwrap()
}

fn request(operation: Value) -> ProbeRequest {
    serde_json::from_value(json!({"protocol_version":1,"iterations":1,
        "tolerance":{"absolute":1e-9,"relative":1e-12,"angular":1e-9},
        "operations":[operation]}))
    .unwrap()
}

#[test]
fn native_curved_refits_certify_with_a_complete_proposed_correspondence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_sets.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let reference = read("observations");
    let mut certified = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        if op["refit_tolerance"].as_f64().unwrap() != 0.001 {
            continue;
        }
        let fitted = &row["value"]["refit_sets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["variant"] == "paired")
            .unwrap()["curves"][0];
        let a = curve(&op["curves"][0]);
        let b = curve(fitted);
        let domain = a.domain();
        let mut pairs = vec![[0., 0.]];
        for i in 1..256 {
            let f = f64::from(i) / 256.;
            let point = b.evaluate(b.parameter_at(f).unwrap()).unwrap();
            let t = a.closest_parameter(point, Tolerance::DEFAULT).unwrap();
            pairs.push([(t - domain.start()) / (domain.end() - domain.start()), f]);
        }
        pairs.push([1., 1.]);
        // The numerical nearest-point search only proposes a monotone map.
        // The exact certificate must then prove every intervening interval.
        assert!(pairs.windows(2).all(|p| p[0][0] < p[1][0]));
        let operation = json!({"op":"curve_deviation_bound","id":op["id"],
            "curves":[op["curves"][0],fitted],"absolute_tolerance":0.001,
            "correspondence":pairs});
        let result = run_request(&request(operation)).unwrap();
        let value = &result.results[0].value;
        assert_eq!(value["before"], value["after"]);
        assert_eq!(value["certified"], true, "{}", op["id"]);
        let bound = value["maximum_deviation_bound"].as_f64().unwrap();
        assert!(bound <= 0.001);
        maximum = maximum.max(bound);
        certified += 1;
        // Original normalized parameters do not describe the fitted clock.
        assert_eq!(a.parameter_curve_deviation_bound(&b, 0.001).unwrap(), None);
    }
    assert_eq!(certified, 3);
    println!("CURVE_DEVIATION native_fits={certified} maximum_bound={maximum}");
}

#[test]
fn json_defaults_zero_tolerance_and_inconclusive_results_are_explicit() {
    let line = json!({"degree":1,"knots":[0.,0.,1.,1.],
        "control_points":[{"point":[0.,0.,0.],"weight":1.},{"point":[1.,0.,0.],"weight":1.}]});
    let op = json!({"op":"curve_deviation_bound","id":"equal","curves":[line,line],"absolute_tolerance":0.});
    let result = run_request(&request(op.clone())).unwrap();
    assert_eq!(result.results[0].value["maximum_deviation_bound"], 0.);
    assert_eq!(
        result.results[0].value["correspondence"],
        json!([[0., 0.], [1., 1.]])
    );
    let mut different = op.clone();
    different["curves"][1]["control_points"][1]["point"][1] = json!(1.);
    let result = run_request(&request(different)).unwrap();
    assert_eq!(
        result.results[0].value["maximum_deviation_bound"],
        Value::Null
    );
    assert_eq!(result.results[0].value["certified"], false);
    for pairs in [json!([]), json!([[0., 0.], [0.5, 1.]])] {
        let mut invalid = op.clone();
        invalid["correspondence"] = pairs;
        assert!(
            run_request(&request(invalid))
                .unwrap_err()
                .to_string()
                .contains("curve correspondence")
        );
    }
    let mut invalid = op;
    invalid["absolute_tolerance"] = json!(-1.);
    assert!(run_request(&request(invalid)).is_err());
}
