#![cfg(feature = "native")]
use std::process::Command;

#[test]
fn native_basis_values_derivatives_and_point_queries_agree_with_independent_limits() {
    let result = Command::new(env!("VIBOCEROS_NATIVE_BASIS_EVALUATION_PROBE"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(
        result.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let rows = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 368);
    for row in &rows {
        assert_eq!(row["basis_correct"], true, "{row}");
        assert_eq!(row["derivatives_correct"], true, "{row}");
        for curve in row["curves"].as_array().unwrap() {
            for key in [
                "points_correct",
                "derivatives_correct",
                "surface_correct",
                "source_unchanged",
            ] {
                assert_eq!(curve[key], true, "{key}: {row}");
            }
        }
    }
    eprintln!(
        "BASIS_EVALUATION stations={} weighted_curve_surface_cases={}",
        rows.len(),
        rows.len() * 3
    );
}
