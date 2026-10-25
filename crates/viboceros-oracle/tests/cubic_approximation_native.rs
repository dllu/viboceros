use serde_json::{Value, json};
use std::path::Path;
use viboceros_geometry::Point3;
use viboceros_oracle::{ProbeRequest, run_request};

fn captured() -> (Value, Value) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/cubic_approximation_sources.json")))
                .unwrap(),
        )
        .unwrap()
    };
    (read("fixtures"), read("observations"))
}

#[test]
fn certified_cubics_retain_sources_and_approximate_all_native_source_positions() {
    let (query, native) = captured();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let holdouts: Value = serde_json::from_slice(
        &std::fs::read(root.join("observations/cubic_approximation_holdouts.json")).unwrap(),
    )
    .unwrap();
    let mut measured = Vec::new();
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let mut samples = row["value"]["samples"].as_array().unwrap().clone();
        assert_eq!(samples.len(), 33);
        let id = format!("{}-holdout", op["id"].as_str().unwrap());
        let held = holdouts["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        assert_eq!(held["value"]["sides"].as_array().unwrap().len(), 11);
        samples.extend(
            held["value"]["sides"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| json!({"parameter":s["parameter"],"point":s["right"]["point"]})),
        );
        for limit in [1e-6, 1e-9] {
            let mut request = query.clone();
            request["operations"] = json!([{
                "op":"curve_cubic_approximation", "id":op["id"], "curve":op["curve"],
                "absolute_tolerance":limit,
                "parameters":samples.iter().map(|s|s["parameter"].clone()).collect::<Vec<_>>(),
            }]);
            let request: ProbeRequest = serde_json::from_value(request).unwrap();
            let result = run_request(&request)
                .unwrap_or_else(|e| panic!("{} limit {limit}: {e:?}", op["id"]));
            let value = &result.results[0].value;
            assert_eq!(value["before"], value["after"]);
            assert_eq!(value["curve"]["degree"], 3);
            assert_eq!(value["curve"]["domain"], value["before"]["domain"]);
            let bound = value["maximum_deviation_bound"].as_f64().unwrap();
            assert!(bound.is_finite() && bound <= limit);
            let count = value["curve"]["control_points"].as_array().unwrap().len();
            assert!(count <= 4096);
            for sample in &samples[33..] {
                assert!(
                    !value["curve"]["knots"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|k| k.as_f64() == sample["parameter"].as_f64()),
                    "{} holdout is an interpolation knot",
                    op["id"]
                );
            }
            let mut maximum = 0_f64;
            let mut maximum_off_grid = 0_f64;
            for (i, (actual, sample)) in value["points"]
                .as_array()
                .unwrap()
                .iter()
                .zip(&samples)
                .enumerate()
            {
                let a =
                    Point3::try_from(serde_json::from_value::<[f64; 3]>(actual.clone()).unwrap())
                        .unwrap();
                let b = Point3::try_from(
                    serde_json::from_value::<[f64; 3]>(sample["point"].clone()).unwrap(),
                )
                .unwrap();
                let error = a.distance_to(b).unwrap();
                assert!(error <= bound + 1e-12, "{}: {error} > {bound}", op["id"]);
                assert!(
                    error <= limit,
                    "{}: native difference {error} > {limit}",
                    op["id"]
                );
                maximum = maximum.max(error);
                if i >= 33 {
                    maximum_off_grid = maximum_off_grid.max(error);
                }
            }
            measured.push(json!({"id":op["id"],"limit":limit,"bound":bound,"maximum_native_sample_difference":maximum,"maximum_off_grid_sample_difference":maximum_off_grid,"controls":count}));
        }
    }
    assert_eq!(measured.len(), 40);
    for limit in [1e-6, 1e-9] {
        assert!(
            measured
                .iter()
                .filter(|r| r["limit"].as_f64() == Some(limit))
                .map(|r| r["maximum_off_grid_sample_difference"].as_f64().unwrap())
                .fold(0_f64, f64::max)
                > limit * 0.01
        );
    }
    println!("CUBIC_APPROXIMATION_NATIVE {}", json!(measured));
}

#[test]
fn invalid_limits_and_sample_requests_fail_before_fitting() {
    let (query, _) = captured();
    for (limit, parameters, reason) in [
        (-1., json!([]), "tolerance"),
        (0., json!([]), "tolerance"),
        (1e-6, json!(vec![0.; 4097]), "4096"),
        (1e-6, json!([-1.]), "outside"),
    ] {
        let mut request = query.clone();
        request["operations"] = json!([{"op":"curve_cubic_approximation","id":"invalid","curve":query["operations"][0]["curve"],"absolute_tolerance":limit,"parameters":parameters}]);
        let request: ProbeRequest = serde_json::from_value(request).unwrap();
        let error = run_request(&request).unwrap_err().to_string();
        assert!(error.contains(reason), "{error}");
    }
}

#[test]
fn default_samples_and_request_tolerance_round_trip_through_the_json_runner() {
    let (mut query, _) = captured();
    query["operations"] = json!([{"op":"curve_cubic_approximation","id":"defaults","curve":query["operations"][0]["curve"]}]);
    let request: ProbeRequest = serde_json::from_value(query).unwrap();
    let result = run_request(&request).unwrap();
    let value = &result.results[0].value;
    assert_eq!(value["parameters"].as_array().unwrap().len(), 33);
    assert_eq!(value["points"].as_array().unwrap().len(), 33);
    let bound = value["maximum_deviation_bound"].as_f64().unwrap();
    // A length-two line needs nonbinary 2/3 and 4/3 cubic handles.
    // Their stored-control rounding is included in the certificate.
    assert!(bound > 0. && bound <= request.tolerance.absolute);
}
