//! Fresh variable-speed NURBS batches use the shared native request policy.
use serde_json::Value;
use viboceros_geometry::{ArcLengthSampler, CurveRef, Point3, Tolerance};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn single_and_multiple_curved_spans_match_public_batches_at_six_precisions() {
    compare("sweep1_refit_clock_resolutions", 6);
}

#[test]
fn spatial_batches_retain_threshold_neighbors_and_the_coarse_request_cap() {
    compare("sweep1_refit_clock_holdouts", 4);
}

fn compare(family: &str, expected_scenarios: usize) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let operations = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!((operations.len(), rows.len()), (5, 5));
    let mut count = 0;
    let mut maximum = 0_f64;
    for (operation, row) in operations.iter().zip(rows) {
        assert_eq!(operation["id"], row["id"]);
        let source = definitions::curve(&operation["curve"]);
        let before = source.clone();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT).unwrap();
        let fractions: Vec<f64> = serde_json::from_value(operation["fractions"].clone()).unwrap();
        let precise = sampler
            .parameters_at_normalized_lengths(&fractions)
            .unwrap();
        let scenarios = operation["inspect_numerical_batch"].as_array().unwrap();
        let batches = row["value"]["numerical_batches"].as_array().unwrap();
        assert_eq!(
            (scenarios.len(), batches.len()),
            (expected_scenarios, expected_scenarios)
        );
        for (scenario, batch) in scenarios.iter().zip(batches) {
            assert_eq!(scenario["id"], batch["id"]);
            assert_eq!(batch["before"], batch["after"]);
            let parameters = sampler
                .parameters_at_incremental_normalized_lengths(
                    &fractions,
                    scenario["fractional_tolerance"].as_f64().unwrap(),
                )
                .unwrap();
            assert_eq!(parameters.len(), 9);
            for (index, parameter) in parameters.into_iter().enumerate() {
                let point = Point3::try_from(
                    serde_json::from_value::<[f64; 3]>(batch["points"][index].clone()).unwrap(),
                )
                .unwrap();
                let error = (parameter - batch["parameters"][index].as_f64().unwrap())
                    .abs()
                    .max(
                        source
                            .evaluate(parameter)
                            .unwrap()
                            .distance_to(point)
                            .unwrap(),
                    );
                assert!(
                    error <= 1e-9,
                    "{} {} {index}: {error}",
                    operation["id"],
                    scenario["id"]
                );
                maximum = maximum.max(error);
                count += 1;
            }
        }
        assert_eq!(
            precise,
            sampler
                .parameters_at_normalized_lengths(&fractions)
                .unwrap()
        );
        assert_eq!(source, before);
    }
    assert_eq!(count, 5 * expected_scenarios * 9);
    println!("CURVED_NURBS_BATCH family={family} queries={count} maximum_difference={maximum}");
}
