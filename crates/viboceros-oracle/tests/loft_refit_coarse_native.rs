//! Every admitted coarse fit must match the complete public native record.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn certified_smooth_candidates_match_native_or_remain_explicitly_unimplemented() {
    let (matched, pending, maximum) = compare_families(
        &[
            "loft_refit_coarse",
            "loft_refit_candidates",
            "loft_refit_candidate_boundary",
            "loft_refit_adjustment",
            "loft_refit_adjustment_fit_clock",
        ],
        1e-9,
    );
    println!("LOFT_REFIT_COARSE matched={matched} pending={pending} maximum={maximum}");
    assert_eq!((matched, pending), (144, 75));
    assert_eq!(matched + pending, 219);
}

#[test]
fn corrected_ellipses_and_commands_match_with_the_documented_numerical_reserve() {
    let (matched, pending, maximum) = compare_families(
        &[
            "loft_refit_adjustment_ellipses",
            "loft_refit_adjustment_local",
            "loft_refit_adjustment_commands",
        ],
        1e-8,
    );
    println!("REFIT_ADJUST_EXTENDED matched={matched} pending={pending} maximum={maximum}");
    assert_eq!((matched, pending), (62, 8));
}

fn compare_families(families: &[&str], epsilon: f64) -> (usize, usize, f64) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut matched = 0;
    let mut pending = 0;
    let mut maximum = 0_f64;
    for family in families {
        let read = |folder| -> Value {
            serde_json::from_slice(
                &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
            )
            .unwrap()
        };
        let query = read("fixtures");
        let native = read("observations");
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
                    expected.as_object_mut().unwrap().remove("refit_adjustment");
                    expected.as_object_mut().unwrap().remove("refitted");
                    let error = comparison::difference(value, &expected)
                        .unwrap_or_else(|error| panic!("{}: {error}", op["id"]));
                    assert!(error <= epsilon, "{}: {error}", op["id"]);
                    if !op["command"].as_bool().unwrap_or(false) {
                        assert_eq!(value["before"], value["after"]);
                    }
                    maximum = maximum.max(error);
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
    }
    (matched, pending, maximum)
}

#[test]
fn public_closest_queries_at_scalar_midpoints_match_native() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_adjustment_queries.json")))
                .unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let actual = run_request(&serde_json::from_value::<ProbeRequest>(query).unwrap()).unwrap();
    let mut maximum = 0_f64;
    for (row, reference) in actual
        .results
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(row.id, reference["id"].as_str().unwrap());
        let error = comparison::difference(&row.value, &reference["value"]).unwrap();
        assert!(error <= 1e-9, "{}: {error}", row.id);
        maximum = maximum.max(error);
    }
    assert_eq!(actual.results.len(), 18);
    println!("REFIT_ADJUST_CLOSEST records=18 maximum={maximum}");
}
