//! Retain the distinction between whole-profile subdivision and private trims.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn subdivision_references_match_or_report_the_unfinished_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_subdivision.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    assert_eq!(query["operations"].as_array().unwrap().len(), 45);
    assert_eq!(native["results"].as_array().unwrap().len(), 45);
    let mut matched = 0;
    let mut pending = 0;
    let mut maximum = 0_f64;
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
                let mut expected = row["value"].clone();
                if let Some(record) = expected.as_object_mut() {
                    record.remove("refit_adjustment");
                }
                let error = comparison::difference(value, &expected)
                    .unwrap_or_else(|error| panic!("{}: {error}", op["id"]));
                let epsilon = if op["op"] == "loft" { 1e-8 } else { 1e-9 };
                assert!(error <= epsilon, "{}: {error}", op["id"]);
                if op["op"] == "loft" {
                    assert_eq!(value["before"], value["after"]);
                }
                maximum = maximum.max(error);
                matched += 1;
            }
            Err(error) => {
                assert_eq!(op["op"], "loft");
                assert!(error.to_string().contains("Loft Refit currently requires"));
                pending += 1;
            }
        }
    }
    println!("REFIT_SUBDIVISION matched={matched} pending={pending} maximum={maximum}");
    assert_eq!((matched, pending), (23, 22));
    assert_eq!(matched + pending, 45);
}
