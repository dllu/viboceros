#![cfg(feature = "native")]

#[test]
fn native_blend_producer_preserves_domain_and_requested_boundary_correspondence() {
    for case in 0..4 {
        for flag in 0..=2 {
            for direction in 1..=2 {
                // Isolation also ensures any future native abort is reported as
                // a failed process instead of terminating the entire harness.
                let output =
                    std::process::Command::new(env!("VIBOCEROS_NATIVE_SURFACE_BLEND_PROBE"))
                        .args([case.to_string(), flag.to_string(), direction.to_string()])
                        .output()
                        .unwrap();
                assert!(
                    output.status.success(),
                    "case={case} flag={flag} direction={direction}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(value["status"], 1000);
                assert_eq!(value["all_finite"], true);
                assert!(value["boundary_correspondence_error"].as_f64().unwrap() < 1e-12);
                if direction == 1 {
                    assert!(value["independent_hermite_error"].as_f64().unwrap() < 1e-12);
                }
            }
        }
    }
}
