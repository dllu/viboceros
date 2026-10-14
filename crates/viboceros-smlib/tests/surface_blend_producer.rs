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

#[test]
fn curved_producer_scales_second_and_third_boundary_jets_by_parameter_speed() {
    for case in 0..4 {
        for flag in 0..=2 {
            for rational in 0..=1 {
                for direction in 1..=2 {
                    for takeoff in [0.2, 0.5, 1.7] {
                        let output = std::process::Command::new(env!(
                            "VIBOCEROS_NATIVE_SURFACE_BLEND_CURVED_PROBE"
                        ))
                        .args([
                            case.to_string(),
                            flag.to_string(),
                            rational.to_string(),
                            direction.to_string(),
                            takeoff.to_string(),
                        ])
                        .output()
                        .unwrap();
                        let context = format!(
                            "case={case} flag={flag} rational={rational} direction={direction} takeoff={takeoff}"
                        );
                        assert!(
                            output.status.success(),
                            "{context}: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                        let value: serde_json::Value =
                            serde_json::from_slice(&output.stdout).unwrap();
                        assert_eq!(value["status"], 1000, "{context}");
                        assert_eq!(value["all_finite"], true, "{context}");
                        let check = |key: &str, epsilon: f64| {
                            let error = value[key].as_f64().unwrap();
                            assert!(error < epsilon, "{context}: {key}={error}");
                        };
                        check("boundary_correspondence_error", 1e-12);
                        if direction == 1 {
                            check("independent_hermite_error", 1e-12);
                        }
                        if flag != 0 {
                            check("tangent_plane_error", 2e-8);
                            assert!(value["minimum_cross_speed"].as_f64().unwrap() > 0.);
                        }
                        if flag == 2 {
                            // Direct samples qualify the chain rule separately
                            // from the producer's subsequent skinning fit.
                            for key in [
                                "sample_first_derivative_error",
                                "sample_second_derivative_error",
                                "sample_third_derivative_error",
                            ] {
                                check(key, 1e-10);
                            }
                            check("cross_normal_curvature_error", 1e-7);
                            check("along_normal_curvature_error", 1e-10);
                            // Mixed curvature is retained as a fitting diagnostic;
                            // it is not covered by the direct sample repair.
                        }
                    }
                }
            }
        }
    }
}
