#[path = "support/compare.rs"]
mod diagnostic_comparison;
#[path = "support/curve_path.rs"]
mod support;
use serde_json::json;
use viboceros_oracle::{ProbeRequest, run_request};

#[test]
fn curve_path_extrusion_matches_licensed_geometry_and_model_history() {
    let mut succeeded = 0;
    let mut rejected = 0;
    for (operation, native) in support::cases() {
        let request: ProbeRequest = serde_json::from_value(json!({"protocol_version":1,"iterations":1,
            "tolerance":{"absolute":1e-6,"relative":1e-12,"angular":1e-8},"operations":[operation]})).unwrap();
        let result = run_request(&request).unwrap();
        let row = &result.results[0];
        let success = native["succeeded"].as_bool().unwrap();
        assert_eq!(
            row.value["succeeded"], success,
            "{}: {:?}",
            row.id, row.value
        );
        if !success {
            rejected += 1;
            assert_eq!(
                row.value["after"],
                json!([{"source":0,"unchanged":true},{"source":1,"unchanged":true}])
            );
            assert!(row.value["undo"].is_null() && row.value["redo"].is_null());
            println!(
                "CURVE_PATH_FAILURE {}",
                json!({"id":row.id,"actual":row.value,"native":native["after"]})
            );
            continue;
        }
        succeeded += 1;
        let maximum_error = support::compare(&row.value, &native);
        println!(
            "CURVE_PATH_COMMAND {}",
            json!({"id":row.id,"value":row.value,"maximum_error":maximum_error})
        );
    }
    assert_eq!((succeeded, rejected), (264, 8));
}
