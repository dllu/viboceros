#![cfg(feature = "native")]

#[test]
fn native_curve_continuity_uses_curvature_vectors_independent_of_parameterization() {
    let output = std::process::Command::new(env!("VIBOCEROS_NATIVE_CURVE_CONTINUITY_PROBE"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 540);
    assert!(rows.iter().all(|r| r["passed"] == true));
    eprintln!("CURVE_CONTINUITY cases={} failures=0", rows.len());
}
