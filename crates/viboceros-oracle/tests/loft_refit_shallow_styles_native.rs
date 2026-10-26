//! Every style/point/closed holdout must match, with no unsupported-case escape.
use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
fn explicit_corners_match_all_styles_point_ends_and_closed_lofts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_shallow_styles.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let actual = run_request(&serde_json::from_value::<ProbeRequest>(query).unwrap()).unwrap();
    let reference = native["results"].as_array().unwrap();
    assert_eq!(actual.results.len(), 14);
    assert_eq!(reference.len(), 14);
    let mut maximum = 0_f64;
    for (row, expected) in actual.results.iter().zip(reference) {
        assert_eq!(row.id, expected["id"]);
        assert_eq!(row.value["before"], row.value["after"]);
        let error = comparison::difference(&row.value, &expected["value"]).unwrap();
        assert!(error <= 1e-9, "{}: {error}", row.id);
        maximum = maximum.max(error);
    }
    println!("SHALLOW_REFIT_STYLES cases=14 maximum={maximum}");
}
