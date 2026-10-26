use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn default_fitting_clock_matches_native_with_reordered_and_repeated_fractions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_fitting_clock.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let request: ProbeRequest = serde_json::from_value(query).unwrap();
    let actual = run_request(&request).unwrap();
    let mut maximum = 0_f64;
    for (a, n) in actual
        .results
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(a.id, n["id"]);
        assert_eq!(a.value["before"], a.value["after"]);
        let error = comparison::difference(&a.value, &n["value"]).unwrap();
        assert!(error <= 1e-9, "{}: {error}", a.id);
        maximum = maximum.max(error);
    }
    assert_eq!(actual.results.len(), 19);
    println!("FITTING_CLOCK records=19 positions=228 maximum={maximum}");
}

#[test]
fn invalid_fraction_counts_and_tolerances_fail_before_sampling() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/loft_fitting_clock.json");
    let query: Value = serde_json::from_slice(&std::fs::read(root).unwrap()).unwrap();
    for (key, value, reason) in [
        ("fractions", json!(vec![0.5; 4097]), "4096"),
        ("fractional_tolerance", json!(-1.), "fractional tolerance"),
        ("fractions", json!([0.5, 1.5]), "fraction"),
    ] {
        let mut op = query["operations"][0].clone();
        op[key] = value;
        let mut q = query.clone();
        q["operations"] = json!([op]);
        let error = run_request(&serde_json::from_value::<ProbeRequest>(q).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains(reason), "{error}");
    }
}

#[test]
fn natural_knot_queries_match_native_across_units_frames_and_plateaus() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/scalar_knot_holdouts.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let request: ProbeRequest = serde_json::from_value(query).unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 36);
    assert_eq!(native["results"].as_array().unwrap().len(), 36);
    let mut maximum = 0_f64;
    for (row, reference) in actual
        .results
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(row.id, reference["id"]);
        assert_eq!(row.value["before"], row.value["after"]);
        let d = comparison::difference(&row.value, &reference["value"])
            .unwrap_or_else(|e| panic!("{}: {e}", row.id));
        assert!(d <= 1e-9, "{}: {d}", row.id);
        maximum = maximum.max(d);
    }
    println!("SCALAR_KNOT_PROTOCOL records=36 stations=396 maximum={maximum}");
}
