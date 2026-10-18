#![cfg(feature = "native")]
use std::process::Command;

#[test]
fn native_tensor_bezier_patches_match_independent_control_nets_and_own_their_data() {
    let result = Command::new(env!("VIBOCEROS_NATIVE_TENSOR_BEZIER_PROBE"))
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
        "{} failures: {:?}\n{}",
        failures.len(),
        failures.iter().take(4).collect::<Vec<_>>(),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(rows.len(), 1008);
    assert!(failures.is_empty());
}
