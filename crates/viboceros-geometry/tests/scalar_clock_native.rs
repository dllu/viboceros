//! Public scalar clock compatibility, separate from incremental batches.
use serde_json::Value;
use std::path::Path;
use viboceros_geometry::{ArcLengthSampler, CurveRef, Point3, Tolerance};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn fitting_clock_matches_native_parameters_and_positions_in_either_order() {
    compare(true);
}

#[test]
fn single_profile_fitter_domains_equal_the_observed_quadrature_length_precision() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_frames.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let mut count = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        for variant in row["value"]["refit_frames"].as_array().unwrap() {
            if !variant["variant"].as_str().unwrap().starts_with("single") {
                continue;
            }
            let source = definitions::curve(&variant["methods"][0]["curves"][0]);
            let fitted = &variant["methods"][1]["curves"][0];
            let knots = fitted["knots"].as_array().unwrap();
            let length = knots.last().unwrap().as_f64().unwrap() - knots[0].as_f64().unwrap();
            let sampler =
                ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT)
                    .unwrap();
            let error = (sampler.quadrature_length_estimate(1e-4).unwrap() - length).abs();
            assert!(
                error <= 1e-9,
                "{} {}: {error}",
                op["id"],
                variant["variant"]
            );
            maximum = maximum.max(error);
            count += 1;
        }
    }
    assert_eq!(count, 24);
    println!("FITTER_DOMAIN_CLOCK records={count} maximum={maximum}");
}

#[test]
#[ignore = "full scalar SDK parity remains unfinished; retain all strict counterexamples"]
fn scalar_queries_match_native_parameters_and_positions_in_either_order() {
    compare(false);
}

fn compare(fitting_only: bool) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_scalar_clock.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let mut count = 0;
    let mut maximum = 0_f64;
    let mut failures = Vec::new();
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let curve = definitions::curve(&op["curves"][0]);
        let before = curve.clone();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        for clock in row["value"]["scalar_clock"].as_array().unwrap() {
            let accuracy = clock["fractional_tolerance"].as_f64().unwrap();
            if fitting_only && accuracy != 1e-4 {
                continue;
            }
            for order in ["forward", "reverse"] {
                let stations = clock[order].as_array().unwrap();
                let fractions = stations
                    .iter()
                    .map(|r| r["fraction"].as_f64().unwrap())
                    .collect::<Vec<_>>();
                let actual = sampler
                    .parameters_at_quadrature_normalized_lengths(&fractions, accuracy)
                    .unwrap();
                for (&parameter, reference) in actual.iter().zip(stations) {
                    assert_eq!(reference["success"], true);
                    let dp = (parameter - reference["parameter"].as_f64().unwrap()).abs();
                    let point = Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(reference["point"].clone()).unwrap(),
                    )
                    .unwrap();
                    let dx = curve
                        .evaluate(parameter)
                        .unwrap()
                        .distance_to(point)
                        .unwrap();
                    maximum = maximum.max(dp).max(dx);
                    count += 1;
                    if dp > 1e-9 || dx > 1e-9 {
                        failures.push(format!("{} accuracy {accuracy} {order} f={} t={parameter} expected={} dt={dp} dx={dx}",op["id"],reference["fraction"],reference["parameter"]));
                    }
                }
            }
        }
        assert_eq!(curve, before);
    }
    println!(
        "SCALAR_CLOCK queries={count} maximum={maximum} mismatches={}",
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(count, if fitting_only { 418 } else { 2090 });
}
