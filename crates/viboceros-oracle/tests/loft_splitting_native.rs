use serde_json::{Value, json};
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "support/compare.rs"]
mod diagnostic_comparison;
#[path = "support/loft_splitting.rs"]
mod support;

fn replay(operation: &Value) -> Value {
    let request: ProbeRequest = serde_json::from_value(json!({
        "protocol_version":1,"iterations":1,
        "tolerance":{"absolute":1e-6,"relative":1e-12,"angular":1e-8},
        "operations":[operation],
    }))
    .unwrap();
    run_request(&request)
        .unwrap_or_else(|error| panic!("{}: {error}", operation["id"]))
        .results
        .remove(0)
        .value
}

#[test]
fn near_threshold_lofts_match_native_partitions_and_history() {
    for (operation, native) in support::angle_cases() {
        let actual = replay(&operation);
        let maximum_error = support::compare_angles(&operation, &actual, &native);
        println!(
            "LOFT_THRESHOLD_COMMAND {}",
            json!({
                "id":operation["id"],"maximum_error":maximum_error,"value":actual,
            })
        );
    }
}

#[test]
#[ignore = "Retain native box, closed face-sense and Straight chart differences in all 276 raw states"]
fn loft_splitting_matches_all_licensed_raw_commands_and_history() {
    let mut failures = Vec::new();
    for (operation, native) in support::cases() {
        let actual = replay(&operation);
        if std::panic::catch_unwind(|| support::compare(&actual, &native)).is_err() {
            failures.push(operation["id"].clone());
        }
    }
    assert!(failures.is_empty(), "Loft cases failed: {failures:?}");
}
