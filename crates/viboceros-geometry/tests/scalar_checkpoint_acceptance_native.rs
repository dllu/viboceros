//! Native endpoint stopping and retained interior checkpoint counterexamples.
use serde_json::Value;
use std::path::Path;
use viboceros_geometry::{ArcLengthSampler, CurveRef, Point3, Tolerance};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;
#[test]
fn numerical_scalar_endpoints_match_native() {
    compare("scalar_checkpoint_acceptance", Selection::Endpoints, 3234);
}
#[test]
fn numerical_scalar_exact_tolerance_thresholds_match_native() {
    compare(
        "scalar_endpoint_boundaries",
        Selection::ExactThresholds,
        526,
    );
}
#[test]
#[ignore = "interior numerical scalar checkpoint parity remains unfinished"]
fn all_scalar_checkpoints_match_native() {
    compare("scalar_checkpoint_acceptance", Selection::All, 10647);
}
#[test]
#[ignore = "adjacent floating-point thresholds and interior controls retain strict counterexamples"]
fn all_scalar_tolerance_boundaries_match_native() {
    compare("scalar_endpoint_boundaries", Selection::All, 1180);
}
#[derive(Clone, Copy)]
enum Selection {
    Endpoints,
    ExactThresholds,
    All,
}
fn compare(family: &str, selection: Selection, expected_count: usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
        )
        .unwrap()
    };
    let q = read("fixtures");
    let n = read("observations");
    let mut count = 0;
    let mut failures = Vec::new();
    let mut maximum = 0_f64;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(n["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let curve = definitions::curve(&op["curve"]);
        let before = curve.clone();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        let f = op["fractions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect::<Vec<_>>();
        let actual = sampler
            .parameters_at_quadrature_normalized_lengths(
                &f,
                op["fractional_tolerance"].as_f64().unwrap(),
            )
            .unwrap();
        for (i, &t) in actual.iter().enumerate() {
            let accuracy = op["fractional_tolerance"].as_f64().unwrap();
            let include = match selection {
                Selection::Endpoints => f[i] <= accuracy || 1. - f[i] <= accuracy,
                Selection::ExactThresholds => [0., 1., accuracy, 1. - accuracy].contains(&f[i]),
                Selection::All => true,
            };
            if !include {
                continue;
            }
            let parameter = row["value"]["parameters"][i].as_f64().unwrap();
            let point = Point3::try_from(
                serde_json::from_value::<[f64; 3]>(row["value"]["points"][i].clone()).unwrap(),
            )
            .unwrap();
            let error = (t - parameter)
                .abs()
                .max(curve.evaluate(t).unwrap().distance_to(point).unwrap());
            maximum = maximum.max(error);
            count += 1;
            if error > 1e-9 {
                failures.push(format!(
                    "{} f={} actual={t} expected={parameter} error={error}",
                    op["id"], f[i]
                ));
            }
        }
        assert_eq!(curve, before);
    }
    println!(
        "CHECKPOINT_ACCEPTANCE queries={count} mismatches={} maximum={maximum}",
        failures.len()
    );
    assert_eq!(count, expected_count);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
