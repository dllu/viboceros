#![cfg(feature = "native")]
//! Compare public native extrema for endpoint-frame candidates and hidden bumps.
use serde_json::Value;
use std::path::Path;
use viboceros_geometry::{CurveRef, Tolerance};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_parameters.rs"]
mod definitions;

#[test]
fn endpoint_frame_deviations_match_native() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_deviation.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let operations = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(operations.len(), 52);
    assert_eq!(rows.len(), 52);
    let tolerance = Tolerance::try_new(1e-10, 1e-12, 1e-8).unwrap();
    let (mut maximum, mut minimum) = (0_f64, 0_f64);
    for (op, row) in operations.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        let first = definitions::curve(&op["curves"][0]);
        let second = definitions::curve(&op["curves"][1]);
        let result = viboceros_smlib::curve_distance_extrema(
            CurveRef::NurbsCurve(&first),
            CurveRef::NurbsCurve(&second),
            [[0., 1.]; 2],
            tolerance,
        )
        .unwrap()
        .unwrap();
        // Equal-distance witnesses can have different parameters. Compare the
        // extrema themselves, rather than prescribing a tied witness.
        let expected = &row["value"]["sdk"];
        let max_error =
            (result.maximum.distance - expected["maximum"]["distance"].as_f64().unwrap()).abs();
        let min_error =
            (result.minimum.distance - expected["minimum"]["distance"].as_f64().unwrap()).abs();
        assert!(max_error <= 1e-12, "{}: {max_error}", op["id"]);
        assert!(min_error <= 1e-12, "{}: {min_error}", op["id"]);
        maximum = maximum.max(max_error);
        minimum = minimum.max(min_error);
    }
    println!("REFIT_DEVIATION records=52 maximum_error={maximum} minimum_error={minimum}");
}
