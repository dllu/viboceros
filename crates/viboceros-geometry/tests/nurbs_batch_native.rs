//! Numerical NURBS boundary selection against immutable public API captures.
use serde_json::Value;
use viboceros_geometry::{ArcLengthSampler, CurveRef, Tolerance};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

fn read(folder: &str, family: &str) -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    serde_json::from_slice(&std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap())
        .unwrap()
}

#[test]
fn fresh_and_prequeried_constant_speed_nurbs_batches_match_native() {
    let mut count = 0;
    let mut maximum = 0_f64;
    let mut failures = Vec::new();
    for (family, expected_operations, expected_scenarios) in [
        ("nurbs_batch_history", 6, 14),
        ("nurbs_batch_resolution", 34, 16),
        ("nurbs_batch_holdouts", 72, 1),
    ] {
        let query = read("fixtures", family);
        let native = read("observations", family);
        assert_eq!(
            query["operations"].as_array().unwrap().len(),
            expected_operations
        );
        assert_eq!(
            query["operations"].as_array().unwrap().len(),
            native["results"].as_array().unwrap().len()
        );
        for (operation, record) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(operation["id"], record["id"]);
            let curve = definitions::curve(&operation["curve"]);
            let before = curve.clone();
            let scenarios = operation["inspect_numerical_batch"].as_array().unwrap();
            let batches = record["value"]["numerical_batches"].as_array().unwrap();
            assert_eq!(scenarios.len(), expected_scenarios);
            assert_eq!(scenarios.len(), batches.len());
            for (scenario, batch) in scenarios.iter().zip(batches) {
                assert_eq!(scenario["id"], batch["id"]);
                assert_eq!(batch["before"], batch["after"]);
                assert_eq!(operation["fractions"], batch["fractions"]);
                let sampler =
                    ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT)
                        .unwrap();
                let fractions: Vec<f64> =
                    serde_json::from_value(batch["fractions"].clone()).unwrap();
                let precise = sampler
                    .parameters_at_normalized_lengths(&fractions)
                    .unwrap();
                for history in scenario["history"].as_array().unwrap() {
                    let accuracy = history["fractional_tolerance"].as_f64().unwrap();
                    match history["query"].as_str().unwrap() {
                        "length" => {
                            sampler.quadrature_length_estimate(accuracy).unwrap();
                        }
                        "scalar" => {
                            sampler
                                .parameter_at_quadrature_normalized_length(0.5, accuracy)
                                .unwrap();
                        }
                        "batch" => {
                            sampler
                                .parameters_at_incremental_normalized_lengths(&fractions, accuracy)
                                .unwrap();
                        }
                        _ => unreachable!(),
                    }
                }
                let actual = sampler
                    .parameters_at_incremental_normalized_lengths(
                        &fractions,
                        scenario["fractional_tolerance"].as_f64().unwrap(),
                    )
                    .unwrap();
                assert_eq!(actual.len(), batch["parameters"].as_array().unwrap().len());
                assert!(actual.windows(2).all(|p| p[0] <= p[1]));
                for (i, (&parameter, expected)) in actual
                    .iter()
                    .zip(batch["parameters"].as_array().unwrap())
                    .enumerate()
                {
                    let expected = expected.as_f64().unwrap();
                    assert!(expected.is_finite());
                    let mut error = (parameter - expected).abs();
                    for (a, b) in curve
                        .evaluate(parameter)
                        .unwrap()
                        .to_array()
                        .iter()
                        .zip(batch["points"][i].as_array().unwrap())
                    {
                        let b = b.as_f64().unwrap();
                        assert!(b.is_finite());
                        error = error.max((a - b).abs());
                    }
                    if error > 1e-9 || (curve.knots().contains(&expected) && parameter != expected)
                    {
                        failures.push(format!("{} {} fraction={} actual={parameter} expected={expected} error={error}",operation["id"],scenario["id"],fractions[i]));
                    }
                    maximum = maximum.max(error);
                    count += 1;
                }
                assert_eq!(
                    precise,
                    sampler
                        .parameters_at_normalized_lengths(&fractions)
                        .unwrap()
                );
            }
            assert_eq!(curve, before);
        }
    }
    println!(
        "NURBS_BATCH queries={count} maximum={maximum} failures={}",
        failures.len()
    );
    assert_eq!(count, 7452);
    assert!(
        failures.is_empty(),
        "{}",
        failures.into_iter().take(20).collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn loft_source_batches_retain_the_native_natural_knots() {
    let mut count = 0;
    let mut maximum = 0_f64;
    for family in ["loft_refit_midpoint_clocks", "nurbs_batch_boundaries"] {
        let query = read("fixtures", family);
        let native = read("observations", family);
        for (operation, record) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(operation["id"], record["id"]);
            let curve = definitions::curve(&operation["curves"][0]);
            let sampler =
                ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT)
                    .unwrap();
            let fractions = (0..=8).map(|i| i as f64 / 8.).collect::<Vec<_>>();
            let actual = sampler
                .parameters_at_incremental_normalized_lengths(
                    &fractions,
                    operation["refit_adjustment_fractional_tolerance"]
                        .as_f64()
                        .unwrap(),
                )
                .unwrap();
            for (&parameter, expected) in actual.iter().zip(
                record["value"]["refit_adjustment"]["parameters"]
                    .as_array()
                    .unwrap(),
            ) {
                let expected = expected.as_f64().unwrap();
                let error = (parameter - expected).abs();
                assert!(
                    error <= 1e-9,
                    "{} {parameter} {expected}: {error}",
                    operation["id"]
                );
                if curve.knots().contains(&expected) {
                    assert_eq!(parameter, expected);
                }
                maximum = maximum.max(error);
                count += 1;
            }
        }
    }
    println!("NURBS_LOFT_BATCH queries={count} maximum={maximum}");
    assert_eq!(count, 900);
}
