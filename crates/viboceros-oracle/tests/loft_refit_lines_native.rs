//! Full native surface records, including parameter domains and all controls.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn straight_refit_matches_native_surfaces_and_preserves_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut matched = 0;
    let mut maximum = 0_f64;
    for family in [
        "loft_refit",
        "loft_refit_policy",
        "loft_refit_lines",
        "loft_refit_line_ends",
    ] {
        let read = |folder| -> Value {
            serde_json::from_slice(
                &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
            )
            .unwrap()
        };
        let query = read("fixtures");
        let reference = read("observations");
        assert_eq!(
            query["operations"].as_array().unwrap().len(),
            reference["results"].as_array().unwrap().len()
        );
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(reference["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let id = op["id"].as_str().unwrap();
            if op["refit_tolerance"].as_f64().unwrap() == 0.
                || (family == "loft_refit" && !id.starts_with("loft-refit-line-"))
                || (family == "loft_refit_policy"
                    && !id.starts_with("loft-refit-policy-line-")
                    && !id.starts_with("loft-refit-policy-uneven_line-"))
            {
                continue;
            }
            let mut input = query.clone();
            input["operations"] = json!([op]);
            let request: ProbeRequest = serde_json::from_value(input).unwrap();
            let actual = run_request(&request)
                .unwrap_or_else(|e| panic!("{id}: {e}"))
                .results[0]
                .value
                .clone();
            let mut expected = row["value"].clone();
            expected.as_object_mut().unwrap().remove("refitted");
            let difference =
                comparison::difference(&actual, &expected).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(difference <= 1e-9, "{id}: {difference}");
            assert_eq!(actual["before"], actual["after"], "{id}");
            matched += 1;
            maximum = maximum.max(difference);
        }
    }
    assert_eq!(matched, 121);
    println!("LOFT_REFIT_LINES matched={matched} maximum={maximum}");
}
