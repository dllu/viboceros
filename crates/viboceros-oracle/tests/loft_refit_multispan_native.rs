//! Every admitted multispan proposal must match complete native surface records.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn multispan_proposals_match_native_or_retain_the_adaptive_policy_gap() {
    compare("loft_refit_multispan", 66, (66, 0));
}

#[test]
fn newly_admitted_command_surfaces_match_native_and_preserve_sources() {
    compare("loft_refit_multispan_commands", 6, (6, 0));
}

fn compare(family: &str, count: usize, expected_counts: (usize, usize)) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    assert_eq!(query["operations"].as_array().unwrap().len(), count);
    assert_eq!(native["results"].as_array().unwrap().len(), count);
    let (mut matched, mut pending, mut maximum) = (0, 0, 0_f64);
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
                expected.as_object_mut().unwrap().remove("refitted");
                let difference = comparison::difference(value, &expected)
                    .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
                assert!(difference <= 1e-9, "{}: {difference}", op["id"]);
                if op["command"] != true {
                    assert_eq!(value["before"], value["after"]);
                } else {
                    assert!(
                        value["originals_present"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|v| v == true)
                    );
                    assert!(
                        value["originals_selected"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|v| v == true)
                    );
                }
                maximum = maximum.max(difference);
                matched += 1;
            }
            Err(error) => {
                assert!(
                    error.to_string().contains("Loft Refit currently requires"),
                    "{}: {error}",
                    op["id"]
                );
                println!("REFIT_MULTISPAN_PENDING {}", op["id"]);
                pending += 1;
            }
        }
    }
    println!("REFIT_MULTISPAN {family} matched={matched} pending={pending} maximum={maximum}");
    assert_eq!((matched, pending), expected_counts);
}
