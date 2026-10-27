//! Curved initial fits across document commands, styles and coordinate frames.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn curved_source_inspection_records_match_or_require_subdivision() {
    compare("loft_refit_curved_clocks", 12, (12, 0));
}

#[test]
fn curved_initial_fits_match_native_styles_commands_and_frames() {
    compare("loft_refit_curved_frames", 64, (64, 0));
}

#[test]
fn a_retraced_line_retains_the_unfinished_native_subdivision_path() {
    compare("loft_refit_projected_order", 4, (2, 2));
}

fn compare(family: &str, count: usize, counts: (usize, usize)) {
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
    let (mut matched, mut pending, mut maximum) = (0, 0, 0_f64);
    for (op, row) in operations.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        let mut op = op.clone();
        op.as_object_mut().unwrap().remove("inspect_refit_sources");
        let mut request = query.clone();
        request["operations"] = json!([op]);
        match run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()) {
            Ok(actual) => {
                let value = &actual.results[0].value;
                let mut expected = row["value"].clone();
                for field in ["refit_sources", "refit_probe_angle_tolerance"] {
                    expected.as_object_mut().unwrap().remove(field);
                }
                let difference = comparison::difference(value, &expected)
                    .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
                assert!(difference <= 1e-9, "{}: {difference}", op["id"]);
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
                maximum = maximum.max(difference);
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
    println!("CURVED_REFIT {family} matched={matched} pending={pending} maximum={maximum}");
    assert_eq!((matched, pending), counts);
}
