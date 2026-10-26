//! Every admitted coarse fit must match the complete public native record.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn initial_smooth_candidates_match_native_or_remain_explicitly_unimplemented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut matched = 0;
    let mut pending = 0;
    let mut maximum = 0_f64;
    for family in [
        "loft_refit_coarse",
        "loft_refit_candidates",
        "loft_refit_candidate_boundary",
    ] {
        let read = |folder| -> Value {
            serde_json::from_slice(
                &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
            )
            .unwrap()
        };
        let query = read("fixtures");
        let native = read("observations");
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let mut request = query.clone();
            request["operations"] = json!([op]);
            match run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()) {
                Ok(actual) => {
                    let value = &actual.results[0].value;
                    let error = comparison::difference(value, &row["value"]).unwrap();
                    assert!(error <= 1e-9, "{}: {error}", op["id"]);
                    assert_eq!(value["before"], value["after"]);
                    maximum = maximum.max(error);
                    matched += 1;
                }
                Err(error) => {
                    assert!(
                        error.to_string().contains("Loft Refit currently requires"),
                        "{}: {error}",
                        op["id"]
                    );
                    pending += 1;
                }
            }
        }
    }
    println!("LOFT_REFIT_COARSE matched={matched} pending={pending} maximum={maximum}");
    assert_eq!((matched, pending), (86, 49));
    assert_eq!(matched + pending, 135);
}
