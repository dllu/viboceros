//! Keep geometric bounds separate from exact native fitting structure.
use serde_json::Value;
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_parameters.rs"]
mod definitions;

#[test]
fn c1_join_has_complete_source_and_native_profile_bounds() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/refit_join_certificate.json");
    let query: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let joined = definitions::curve(&query["operations"][0]["curves"][0]);
    let before = joined.clone();
    let reduced = joined
        .try_reduce_knot_multiplicity_with_bound(0.5, 2, 1e-6)
        .unwrap()
        .unwrap();
    assert_eq!(reduced.removed, 1);
    assert_eq!(reduced.curve.control_points().len(), 6);
    assert_eq!(joined, before);
    let witness = definitions::curve(&query["operations"][0]["curves"][1]);
    assert!(
        witness
            .parameter_curve_deviation_bound(&reduced.curve, 1e-12)
            .unwrap()
            .is_some()
    );
    let native = definitions::curve(&query["operations"][1]["curves"][0]);
    let native_bound = native
        .parameter_curve_deviation_bound(&reduced.curve, 1e-6)
        .unwrap()
        .unwrap();
    assert!(native_bound <= 1e-6);
    let control_difference = native
        .control_points()
        .iter()
        .zip(reduced.curve.control_points())
        .map(|(a, b)| a.point().distance_to(b.point()).unwrap())
        .fold(0_f64, f64::max);
    assert!(control_difference > 1e-8);
    let actual = run_request(&serde_json::from_value::<ProbeRequest>(query).unwrap()).unwrap();
    assert_eq!(actual.results.len(), 3);
    for row in &actual.results {
        assert_eq!(row.value["certified"], true, "{}", row.id);
        assert_eq!(row.value["before"], row.value["after"]);
    }
    println!(
        "REFIT_JOIN native_profile_bound={native_bound} control_difference={control_difference}"
    );
    println!("REFIT_JOIN_CERTIFICATES {}",serde_json::to_string(&actual.results.iter()
        .map(|row|serde_json::json!({"id":row.id,"bound":row.value["maximum_deviation_bound"]}))
        .collect::<Vec<_>>()).unwrap());
}
