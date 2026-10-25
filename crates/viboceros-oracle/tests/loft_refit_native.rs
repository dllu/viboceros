//! Curved Refit must not silently run an ordinary Loft.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

fn fixture() -> (Value, Value) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit.json"))).unwrap(),
        )
        .unwrap()
    };
    (read("fixtures"), read("observations"))
}

#[test]
fn zero_refit_matches_all_native_ordinary_lofts_and_preserves_sources() {
    let (query, native) = fixture();
    let mut count = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        if op["refit_tolerance"].as_f64() != Some(0.) {
            continue;
        }
        assert_eq!(op["id"], row["id"]);
        let mut request = query.clone();
        request["operations"] = json!([op]);
        let request: ProbeRequest = serde_json::from_value(request).unwrap();
        let actual = run_request(&request).unwrap().results[0].value.clone();
        let difference = comparison::difference(&actual, &row["value"])
            .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        assert!(difference <= 1e-9, "{}: {difference}", op["id"]);
        assert_eq!(actual["before"], actual["after"]);
        count += 1;
        maximum = maximum.max(difference);
    }
    assert_eq!(count, 25);
    println!("LOFT_REFIT_ZERO matched={count} maximum={maximum}");
}

#[test]
fn curved_refit_reports_the_missing_policy_instead_of_false_geometry() {
    let (query, _) = fixture();
    let mut count = 0;
    for op in query["operations"].as_array().unwrap() {
        if op["refit_tolerance"].as_f64().unwrap() <= 0.
            || op["id"].as_str().unwrap().starts_with("loft-refit-line-")
        {
            continue;
        }
        for command in [false, true] {
            let mut op = op.clone();
            op["command"] = json!(command);
            let mut request = query.clone();
            request["operations"] = json!([op]);
            let request: ProbeRequest = serde_json::from_value(request).unwrap();
            let error = run_request(&request).unwrap_err().to_string();
            assert!(error.contains("Loft Refit currently requires"), "{error}");
            count += 1;
        }
    }
    assert_eq!(count, 120);
}

#[test]
fn invalid_and_conflicting_simplification_is_rejected_before_geometry() {
    let (query, _) = fixture();
    for (refit, rebuild, reason) in [
        (-1., 0, "nonnegative finite"),
        (0., 4, "mutually exclusive"),
        (0.1, 4, "mutually exclusive"),
    ] {
        let mut op = query["operations"][0].clone();
        op["curves"] = json!([]); // Invalid geometry must never be reached.
        op["refit_tolerance"] = json!(refit);
        op["rebuild_point_count"] = json!(rebuild);
        let mut request = query.clone();
        request["operations"] = json!([op]);
        let request: ProbeRequest = serde_json::from_value(request).unwrap();
        let error = run_request(&request).unwrap_err().to_string();
        assert!(error.contains(reason), "{error}");
    }
}
