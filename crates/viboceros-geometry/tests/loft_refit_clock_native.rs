//! Isolate the already-public length primitives from unknown Loft fitting policy.
use serde_json::Value;
use std::path::Path;
use viboceros_geometry::{ArcLengthSampler, CurveRef, Tolerance};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn native_length_estimates_and_returned_positions_match_profile_observations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_clock.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let mut maximum = 0_f64;
    let mut stations = 0;
    let mut lengths = 0;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let curve = definitions::curve(&op["curves"][0]);
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        for reference in row["value"]["refit_clock"]["length_queries"]
            .as_array()
            .unwrap()
        {
            let accuracy = reference["fractional_tolerance"].as_f64().unwrap();
            let error = (sampler.quadrature_length_estimate(accuracy).unwrap()
                - reference["length"].as_f64().unwrap())
            .abs();
            assert!(error <= 1e-9, "{} length at {accuracy}: {error}", op["id"]);
            maximum = maximum.max(error);
            lengths += 1;
            for station in reference["stations"].as_array().unwrap() {
                assert_eq!(station["success"], true);
                let fraction = station["fraction"].as_f64().unwrap();
                // Use the reference's parameter. Scalar inversion and the
                // existing incremental batch are distinct numerical contracts.
                let parameter = station["parameter"].as_f64().unwrap();
                let mut error = 0_f64;
                for (actual, expected) in curve
                    .evaluate(parameter)
                    .unwrap()
                    .to_array()
                    .iter()
                    .zip(station["point"].as_array().unwrap())
                {
                    error = error.max((actual - expected.as_f64().unwrap()).abs());
                }
                assert!(
                    error <= 1e-9,
                    "{} station {fraction} at {accuracy}: {error}",
                    op["id"]
                );
                maximum = maximum.max(error);
                stations += 1;
            }
        }
    }
    assert_eq!((lengths, stations), (63, 441));
    println!("LOFT_REFIT_CLOCK lengths={lengths} stations={stations} maximum={maximum}");
}

#[test]
fn scalar_normalized_length_and_incremental_batch_are_distinct_contracts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let query: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/loft_refit_clock.json")).unwrap(),
    )
    .unwrap();
    let native: Value = serde_json::from_slice(
        &std::fs::read(root.join("observations/loft_refit_clock.json")).unwrap(),
    )
    .unwrap();
    let index = query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .position(|o| o["id"] == "loft-refit-clock-cubic-0.1")
        .unwrap();
    let curve = definitions::curve(&query["operations"][index]["curves"][0]);
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let reference =
        &native["results"][index]["value"]["refit_clock"]["length_queries"][2]["stations"][0];
    assert_eq!(reference["fraction"], 0.125);
    let batch = sampler
        .parameters_at_incremental_normalized_lengths(&[0.125], 0.01)
        .unwrap()[0];
    let scalar = reference["parameter"].as_f64().unwrap();
    let difference = curve
        .evaluate(batch)
        .unwrap()
        .distance_to(curve.evaluate(scalar).unwrap())
        .unwrap();
    assert!(
        difference > 0.01,
        "retain the scalar/batch counterexample: {difference}"
    );
    println!("LOFT_REFIT_SCALAR_BATCH position_difference={difference}");
}
