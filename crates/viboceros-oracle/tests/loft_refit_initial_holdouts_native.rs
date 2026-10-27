//! Curved source holdouts must reproduce native fits or retain explicit gaps.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn curved_multispan_initial_fits_match_complete_native_records() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_initial_holdouts.json")))
                .unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let operations = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(operations.len(), 24);
    assert_eq!(rows.len(), 24);
    let (mut matched, mut pending, mut pending_initial, mut maximum) = (0, 0, 0, 0_f64);
    for (op, row) in operations.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        let mut request = query.clone();
        request["operations"] = json!([op]);
        match run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()) {
            Ok(actual) => {
                let value = &actual.results[0].value;
                assert_eq!(value["before"], value["after"]);
                let difference = comparison::difference(value, &row["value"])
                    .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
                assert!(difference <= 1e-9, "{}: {difference}", op["id"]);
                maximum = maximum.max(difference);
                matched += 1;
            }
            Err(error) => {
                assert!(error.to_string().contains("Loft Refit currently requires"));
                println!(
                    "CURVED_INITIAL_PENDING {} native_controls={}",
                    op["id"], row["value"]["surfaces"][0]["control_count"][1]
                );
                pending += 1;
                pending_initial +=
                    usize::from(row["value"]["surfaces"][0]["control_count"][1] == 4);
            }
        }
    }
    println!("CURVED_INITIAL_REFIT matched={matched} pending={pending} maximum={maximum}");
    // Only the six native results needing additional pieces remain unsupported.
    assert_eq!((matched, pending, pending_initial), (18, 6, 0));
}
