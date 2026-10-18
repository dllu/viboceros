#![cfg(feature = "native")]

#[test]
fn circular_fillets_preserve_the_same_parameter_on_both_rails() {
    let result = std::process::Command::new(env!("VIBOCEROS_NATIVE_CIRCULAR_FILLET_RAILS_PROBE"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let cases: Vec<serde_json::Value> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(cases.len(), 216);
    for case in cases {
        assert_eq!(case["passed"], true, "{case}");
        let tolerance = case["tolerance"].as_f64().unwrap();
        for measure in [
            "maximum_pair_gap",
            "maximum_source_gap",
            "maximum_torus_error",
        ] {
            assert!(case[measure].as_f64().unwrap() <= tolerance, "{case}");
        }
    }
}
