//! Explicit angle overload: compare complete records and keep remaining gaps.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn refit_angles_match_native_definitions_or_report_adaptive_fitting_gaps() {
    compare("loft_refit_angles", 176, (157, 19));
}

#[test]
fn higher_degree_charts_and_smooth_holdouts_match_native() {
    compare("loft_refit_angle_holdouts", 41, (35, 6));
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
                let actual = &actual.results[0].value;
                let mut expected = row["value"].clone();
                expected.as_object_mut().unwrap().remove("refitted");
                let error = comparison::difference(actual, &expected)
                    .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
                assert!(error <= 1e-9, "{}: {error}", op["id"]);
                assert_eq!(actual["before"], actual["after"]);
                maximum = maximum.max(error);
                matched += 1;
            }
            Err(error) => {
                assert!(
                    op["refit_tolerance"].as_f64().unwrap() > 0.,
                    "{}: {error}",
                    op["id"]
                );
                assert!(
                    error.to_string().contains("Loft Refit currently requires"),
                    "{}: {error}",
                    op["id"]
                );
                println!("LOFT_REFIT_ANGLE_PENDING {}", op["id"]);
                pending += 1;
            }
        }
    }
    println!("LOFT_REFIT_ANGLES {family} matched={matched} pending={pending} maximum={maximum}");
    assert_eq!((matched, pending), expected_counts);
}

#[test]
fn command_angle_overrides_are_rejected_before_geometry_instead_of_ignored() {
    let request = json!({"protocol_version":1, "operations":[{
        "op":"loft", "id":"angle-command", "curves":[], "command":true,
        "refit_tolerance":0.001, "refit_angle_tolerance":0.5
    }]});
    let error = run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()).unwrap_err();
    assert!(error.to_string().contains("API only"), "{error}");
}
