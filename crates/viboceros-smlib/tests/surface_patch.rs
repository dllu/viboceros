#![cfg(feature = "native")]
use std::process::Command;

#[test]
fn native_surface_patches_restrict_the_exact_tensor_region_and_own_their_data() {
    let result = Command::new(env!("VIBOCEROS_NATIVE_SURFACE_PATCH_PROBE"))
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
    assert_eq!(rows.len(), 256);
    for row in &rows {
        assert_eq!(row["source_unchanged"], true, "{row}");
        assert_eq!(row["independent_ownership"], true, "{row}");
        for output in row["outputs"].as_array().unwrap() {
            assert_eq!(output["correct"], true, "{row}");
        }
    }
    eprintln!("SURFACE_PATCH cases={}", rows.len());
}
