//! Initial multispan fitting must reproduce full native API and command records.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn partial_multispan_profiles_match_native_complete_surfaces() {
    compare("loft_refit_interval_sampling", 12);
    compare("loft_refit_interval_clocks", 20);
}

#[test]
fn initial_multispan_fits_preserve_native_styles_representations_and_commands() {
    compare("loft_refit_initial_stations", 50);
}

fn compare(family: &str, count: usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let operations = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(operations.len(), count);
    assert_eq!(rows.len(), count);
    let mut maximum = 0_f64;
    for (op, row) in operations.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        let mut request = query.clone();
        request["operations"] = json!([op]);
        let actual = run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        let value = &actual.results[0].value;
        let difference = comparison::difference(value, &row["value"])
            .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        assert!(difference <= 1e-9, "{}: {difference}", op["id"]);
        maximum = maximum.max(difference);
        if op["command"] != true {
            assert_eq!(value["before"], value["after"]);
        } else {
            assert_eq!(value["succeeded"], true);
            for field in ["originals_present", "originals_selected"] {
                assert!(value[field].as_array().unwrap().iter().all(|v| v == true));
            }
            assert_eq!(value["outputs"].as_array().unwrap().len(), 1);
            assert_eq!(value["outputs"][0]["valid"], true);
        }
    }
    println!("INTERVAL_REFIT {family} records={count} maximum={maximum}");
}
