//! Natural-knot acceptance belongs to numerical scalar length queries.
use serde_json::Value;
use viboceros_geometry::{ArcLengthSampler, CurveRef, Tolerance};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn scalar_midpoint_clocks_preserve_native_natural_knots() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_midpoint_clocks.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let (mut count, mut maximum) = (0, 0_f64);
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
            for order in ["forward", "reverse"] {
                let stations = clock[order].as_array().unwrap();
                let fractions = stations
                    .iter()
                    .map(|s| s["fraction"].as_f64().unwrap())
                    .collect::<Vec<_>>();
                let actual = sampler
                    .parameters_at_quadrature_normalized_lengths(&fractions, accuracy)
                    .unwrap();
                for (parameter, station) in actual.iter().zip(stations) {
                    assert_eq!(station["success"], true);
                    let mut error = (parameter - station["parameter"].as_f64().unwrap()).abs();
                    for (a, b) in curve
                        .evaluate(*parameter)
                        .unwrap()
                        .to_array()
                        .into_iter()
                        .zip(station["point"].as_array().unwrap())
                    {
                        error = error.max((a - b.as_f64().unwrap()).abs());
                    }
                    assert!(
                        error <= 1e-9,
                        "{} {accuracy} {}: {error}",
                        op["id"],
                        station["fraction"]
                    );
                    maximum = maximum.max(error);
                    count += 1;
                }
            }
        }
        assert_eq!(curve, before);
    }
    println!("MIDPOINT_SCALAR stations={count} maximum={maximum}");
    assert_eq!(count, 3960);
}

#[test]
fn independent_scalar_holdouts_match_frames_units_weights_and_plateaus() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/scalar_knot_holdouts.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let (mut count, mut maximum) = (0, 0_f64);
    assert_eq!(query["operations"].as_array().unwrap().len(), 36);
    assert_eq!(native["results"].as_array().unwrap().len(), 36);
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let curve = definitions::curve(&op["curve"]);
        let before = curve.clone();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        let fractions = serde_json::from_value::<Vec<f64>>(op["fractions"].clone()).unwrap();
        let actual = sampler
            .parameters_at_quadrature_normalized_lengths(
                &fractions,
                op["fractional_tolerance"].as_f64().unwrap(),
            )
            .unwrap();
        assert_eq!(row["value"]["before"], row["value"]["after"]);
        for (i, parameter) in actual.iter().enumerate() {
            let mut error = (parameter - row["value"]["parameters"][i].as_f64().unwrap()).abs();
            for (a, b) in curve
                .evaluate(*parameter)
                .unwrap()
                .to_array()
                .into_iter()
                .zip(row["value"]["points"][i].as_array().unwrap())
            {
                error = error.max((a - b.as_f64().unwrap()).abs());
            }
            assert!(error <= 1e-9, "{} {}: {error}", op["id"], fractions[i]);
            maximum = maximum.max(error);
            count += 1;
        }
        assert_eq!(curve, before);
    }
    println!("SCALAR_KNOT_HOLDOUT stations={count} maximum={maximum}");
    assert_eq!(count, 396);
}
