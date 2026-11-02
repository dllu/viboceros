use serde_json::json;
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "support/compare.rs"]
mod diagnostic_comparison;
#[path = "support/brep_direction.rs"]
mod support;

#[test]
fn dir_commands_match_shared_native_geometry_selection_and_history() {
    replay(false);
}
#[test]
fn selected_face_charts_match_public_native_face_edits() {
    replay(true);
}
fn replay(sdk: bool) {
    replay_cases(support::cases(sdk));
}
#[test]
fn provisional_sessions_match_native_accept_cancel_and_noop_history() {
    replay_cases(support::session_cases());
}
fn replay_cases(cases: Vec<(serde_json::Value, serde_json::Value)>) {
    let mut failures = Vec::new();
    for (op, native) in cases {
        let request: ProbeRequest =
            serde_json::from_value(json!({"protocol_version":1,"iterations":1,
            "tolerance":{"absolute":1e-8,"relative":1e-12,"angular":1e-8},"operations":[op]}))
            .unwrap();
        let response = match run_request(&request) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{}: {e}", op["id"]));
                continue;
            }
        };
        let actual = &response.results[0].value;
        if std::panic::catch_unwind(|| support::compare(actual, &native)).is_err() {
            failures.push(op["id"].to_string());
        }
    }
    assert!(failures.is_empty(), "Dir comparisons failed: {failures:?}");
}
