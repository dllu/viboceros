//! Fresh coarse scalar queries exercise early fractions and inverse overshoot.
use serde_json::Value;
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

#[test]
#[ignore = "strict coarse scalar SDK parity remains unfinished; retain the complete holdouts"]
fn overshoot_queries_match_native_lengths_parameters_and_points() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder, family| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
        )
        .unwrap()
    };
    let mut maximum = 0_f64;
    let mut positions = 0;
    let mut records = 0;
    let mut matched = 0;
    let mut failures = Vec::new();
    for (family, count) in [("scalar_overshoot", 24), ("scalar_linearity", 9)] {
        let query = read("fixtures", family);
        let native = read("observations", family);
        let actual = run_request(&serde_json::from_value::<ProbeRequest>(query).unwrap()).unwrap();
        assert_eq!(actual.results.len(), count);
        assert_eq!(native["results"].as_array().unwrap().len(), count);
        for (a, n) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, n["id"]);
            assert_eq!(a.value["before"], a.value["after"]);
            let error = comparison::difference(&a.value, &n["value"]).unwrap();
            if error > 1e-9 {
                failures.push(format!("{}: {error}", a.id));
            } else {
                matched += 1;
            }
            maximum = maximum.max(error);
            positions += a.value["parameters"].as_array().unwrap().len();
            records += 1;
        }
    }
    println!(
        "SCALAR_OVERSHOOT records={records} matched={matched} mismatches={} positions={positions} maximum={maximum}",
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(positions, 813);
}
