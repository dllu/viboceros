//! Independently reproduce the inputs and public continuity of cubic diagnostics.
use serde_json::Value;
use viboceros_geometry::{
    ArcLengthSampler, CurveContinuity, CurveParametricContinuity, CurveRef, Tolerance,
};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

fn captured() -> impl Iterator<Item = (Value, Value)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    [
        "sweep1_refit_cubic_structure",
        "sweep1_refit_cubic_structure_default_angle",
    ]
    .into_iter()
    .flat_map(move |family| {
        let read = |folder| -> Value {
            serde_json::from_slice(
                &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
            )
            .unwrap()
        };
        let (query, observed) = (read("fixtures"), read("observations"));
        let operations = query["operations"].as_array().unwrap();
        let rows = observed["results"].as_array().unwrap();
        assert_eq!((operations.len(), rows.len()), (48, 48));
        operations
            .iter()
            .cloned()
            .zip(rows.iter().cloned())
            .collect::<Vec<_>>()
    })
}

#[test]
fn parametric_and_geometric_breaks_match_public_rhino_on_whole_and_trimmed_inputs() {
    let mut count = 0;
    for (operation, row) in captured() {
        assert_eq!(operation["id"], row["id"]);
        let curve = definitions::curve(&row["value"]["source"]);
        let before = curve.clone();
        for (name, kind) in [
            ("C0_continuous", CurveParametricContinuity::C0),
            ("C1_continuous", CurveParametricContinuity::C1),
            ("C2_continuous", CurveParametricContinuity::C2),
        ] {
            let actual = curve.parametric_discontinuity_parameters(kind).unwrap();
            let native: Vec<f64> =
                serde_json::from_value(row["value"]["discontinuities"][name].clone()).unwrap();
            assert_eq!(actual, native, "{} {name}", operation["id"]);
            count += 1;
        }
        for (name, kind) in [
            ("G1_continuous", CurveContinuity::G1),
            ("G2_continuous", CurveContinuity::G2),
        ] {
            let actual = curve
                .discontinuity_parameters(kind, 1_f64.to_radians())
                .unwrap();
            let native: Vec<f64> =
                serde_json::from_value(row["value"]["discontinuities"][name].clone()).unwrap();
            assert_eq!(actual, native, "{} {name}", operation["id"]);
            count += 1;
        }
        assert_eq!(curve, before);
    }
    assert_eq!(count, 480);
}

#[test]
fn independent_lookup_and_trimming_preserve_complete_native_input_images() {
    let mut count = 0;
    let mut maximum = 0_f64;
    for (operation, row) in captured() {
        assert_eq!(operation["id"], row["id"]);
        let source = definitions::curve(&operation["curve"]);
        let before = source.clone();
        let target = definitions::curve(&row["value"]["source"]);
        let actual = if let Some(pair) = operation["lookup_interval"].as_array() {
            let sampler =
                ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT)
                    .unwrap();
            let fractions = std::array::from_fn::<_, 9, _>(|i| i as f64 / 8.);
            let lookup = sampler
                .parameters_at_incremental_normalized_lengths(&fractions, 0.01)
                .unwrap();
            let parameter = |f: f64| {
                let station = f * 8.;
                let index = (station.floor() as usize).min(7);
                let blend = station - index as f64;
                lookup[index].mul_add(1. - blend, lookup[index + 1] * blend)
            };
            let parameters = [
                parameter(pair[0].as_f64().unwrap()),
                parameter(pair[1].as_f64().unwrap()),
            ];
            for (index, t) in parameters.iter().enumerate() {
                assert!(
                    (t - row["value"]["interval_parameters"][index].as_f64().unwrap()).abs() < 1e-9
                );
            }
            source.try_trimmed(parameters[0]..=parameters[1]).unwrap()
        } else {
            source.clone()
        };
        let bound = actual
            .parameter_curve_deviation_bound(&target, 1e-9)
            .unwrap()
            .unwrap_or_else(|| panic!("{} complete input image", operation["id"]));
        maximum = maximum.max(bound);
        assert_eq!(source, before);
        count += 1;
    }
    assert_eq!(count, 96);
    println!("CUBIC_STRUCTURE_INPUTS cases={count} complete_bound={maximum}");
}
