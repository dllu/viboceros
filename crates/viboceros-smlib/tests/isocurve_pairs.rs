#![cfg(feature = "native")]
use std::process::Command;

#[test]
fn native_isocurve_pairs_preserve_the_complete_basis_and_coordinate_parameters() {
    let result = Command::new(env!("VIBOCEROS_NATIVE_ISOCURVE_PAIR_PROBE"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(
        result.status.success(),
        "native pair probe failed: {}\n{stdout}\n{}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    let rows = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 16);
    for row in &rows {
        assert_eq!(row["exact_curve"], true, "{row}");
        assert_eq!(row["uv_matches"], true, "{row}");
        assert_eq!(row["source_unchanged"], true, "{row}");
        assert_eq!(row["independent_ownership"], true, "{row}");
        assert_eq!(row["actual_knots"], row["expected_knots"], "{row}");
        assert_eq!(
            row["actual_homogeneous"], row["expected_homogeneous"],
            "{row}"
        );
    }
    eprintln!("ISOCURVE_PAIRS cases={}", rows.len());
}

#[test]
fn rejected_native_extraction_leaves_no_partial_pair_or_source_changes() {
    let result = Command::new(env!("VIBOCEROS_NATIVE_ISOCURVE_PAIR_PROBE"))
        .arg("--discontinuity")
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let row: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(row["valid_outcome"], true);
    assert_eq!(row["source_unchanged"], true);
    if row["status"] != 1000 {
        assert_eq!(row["curve_present"], false);
        assert_eq!(row["uv_present"], false);
    }
    eprintln!("ISOCURVE_DISCONTINUITY {row}");
}
