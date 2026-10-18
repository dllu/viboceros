#![cfg(feature = "native")]
use std::process::Command;

#[test]
fn native_given_parameter_interpolation_recovers_independent_curves_and_owns_output() {
    let result = Command::new(env!("VIBOCEROS_NATIVE_POINT_INTERPOLATION_PROBE"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(result.stdout).unwrap();
    let rows = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    let failures = rows
        .iter()
        .filter(|row| row["passed"] != true)
        .collect::<Vec<_>>();
    assert!(
        result.status.success(),
        "{} failures: {failures:?}\n{}",
        failures.len(),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(rows.len(), 2053);
    assert_eq!(rows.iter().filter(|row| row["invalid"] == true).count(), 13);
    assert!(failures.is_empty());
}
