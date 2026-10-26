//! Scalar queries on an active interval preserve the original source chart.
use serde_json::{Value, json};
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

fn read(folder: &str) -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    serde_json::from_slice(
        &std::fs::read(root.join(format!("{folder}/interval_length_clocks.json"))).unwrap(),
    )
    .unwrap()
}

#[test]
fn interval_scalar_clocks_match_native_without_reparameterizing_or_mutating_the_source() {
    let query = read("fixtures");
    let native = read("observations");
    let actual =
        run_request(&serde_json::from_value::<ProbeRequest>(query.clone()).unwrap()).unwrap();
    assert_eq!(actual.results.len(), 96);
    assert_eq!(native["results"].as_array().unwrap().len(), 96);
    let (mut stations, mut maximum) = (0, 0_f64);
    for ((op, row), reference) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(actual.results)
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(row.id, reference["id"]);
        assert_eq!(op["id"], row.id);
        assert_eq!(row.value["before"], row.value["after"]);
        assert_eq!(row.value["domain"], op["subdomain"]);
        let parameters = row.value["parameters"].as_array().unwrap();
        assert_eq!(parameters[0], op["subdomain"][0]);
        assert_eq!(parameters[8], op["subdomain"][1]);
        assert_eq!(parameters[4], parameters[9]);
        assert_eq!(parameters[2], parameters[10]);
        let difference = comparison::difference(&row.value, &reference["value"])
            .unwrap_or_else(|e| panic!("{}: {e}", row.id));
        assert!(difference <= 1e-9, "{}: {difference}", row.id);
        maximum = maximum.max(difference);
        stations += parameters.len();
    }
    println!("INTERVAL_CLOCK records=96 stations={stations} maximum={maximum}");
    assert_eq!(stations, 1056);
}

#[test]
fn interval_queries_reject_empty_reversed_or_outside_ranges_and_keep_the_default_contract() {
    let query = read("fixtures");
    let mut operation = query["operations"][0].clone();
    for interval in [[0.2, 0.2], [0.4, 0.2], [-0.1, 0.4], [0.2, 1.1]] {
        operation["subdomain"] = json!(interval);
        let mut request = query.clone();
        request["operations"] = json!([operation]);
        let error = run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("invalid scalar length subdomain"), "{error}");
    }
    operation.as_object_mut().unwrap().remove("subdomain");
    let mut request = query.clone();
    request["operations"] = json!([operation.clone()]);
    let full =
        run_request(&serde_json::from_value::<ProbeRequest>(request.clone()).unwrap()).unwrap();
    assert_eq!(full.results[0].value["domain"], json!([0., 1.]));
    operation["subdomain"] = json!([0., 1.]);
    request["operations"] = json!([operation]);
    let explicit = run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()).unwrap();
    assert_eq!(full.results[0].value, explicit.results[0].value);
}
