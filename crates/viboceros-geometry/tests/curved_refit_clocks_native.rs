//! Frozen native fitting clocks and certificates for retained initial cubics.
use serde_json::Value;
use std::path::Path;
use viboceros_geometry::{ArcLengthSampler, CurveRef, Tolerance};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn curved_fitting_batches_match_native_and_complete_initial_images_are_certified() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_curved_clocks.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let operations = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(operations.len(), 12);
    assert_eq!(rows.len(), 12);
    let fractions = std::array::from_fn::<_, 9, _>(|i| i as f64 / 8.);
    let (mut maximum, mut certified, mut half_certified) = (0_f64, 0, 0);
    for (op, row) in operations.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        let source = definitions::curve(&op["curves"][0]);
        let before = source.clone();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT).unwrap();
        let parameters = sampler
            .parameters_at_incremental_normalized_lengths(&fractions, 1e-4)
            .unwrap();
        let paired = &row["value"]["refit_sources"][1];
        let clock = &paired["numerical_clocks"][3];
        assert_eq!(clock["fractional_tolerance"], 1e-2);
        for (i, parameter) in parameters.iter().enumerate() {
            let error = (parameter - clock["batch_parameters"][i].as_f64().unwrap()).abs();
            assert!(error <= 1e-12, "{}: {i}: {error}", op["id"]);
            maximum = maximum.max(error);
        }
        let fitted = &paired["curves"][0];
        if fitted["control_points"].as_array().unwrap().len() == 4 {
            let candidate = definitions::curve(fitted);
            let tolerance = op["refit_tolerance"].as_f64().unwrap();
            assert!(
                source
                    .projected_curve_deviation_bound(&candidate, tolerance)
                    .unwrap()
                    .is_some(),
                "{}",
                op["id"]
            );
            certified += 1;
            half_certified += usize::from(
                source
                    .projected_curve_deviation_bound(&candidate, tolerance * 0.5)
                    .unwrap()
                    .is_some(),
            );
        }
        assert_eq!(source, before);
    }
    println!(
        "CURVED_CLOCKS records=12 parameters=108 maximum_parameter_error={maximum} certified={certified} half_certified={half_certified}"
    );
    assert_eq!((certified, half_certified), (6, 4));
}
