use serde_json::json;
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "support/direction_analysis.rs"]
mod support;

#[test]
fn showdir_commands_controls_and_lifecycle_match_native_model_and_display_state() {
    for (operation, native) in support::cases() {
        let request:ProbeRequest=serde_json::from_value(json!({"protocol_version":1,"iterations":1,"tolerance":{"absolute":1e-8,"relative":1e-12,"angular":1e-8},"operations":[operation]})).unwrap();
        let response = run_request(&request).unwrap();
        support::compare(
            &response.results[0].value,
            &native,
            operation["id"].as_str().unwrap(),
        );
    }
}
