//! Isolate piece construction from subdivision selection using observed cuts.
//! Production fitting never reads these cuts or any native control net.
use super::*;
use crate::{ArcLengthSampler, CurveRef, Tolerance};
use serde_json::Value;

#[test]
fn independently_constructed_pieces_match_complete_native_images_at_observed_cuts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |path: &str| -> Value {
        serde_json::from_slice(&std::fs::read(root.join(path)).unwrap()).unwrap()
    };
    let sources = read("capture_sources/sweep1_refit_c1_profiles.json");
    let cuts = read("capture_sources/sweep1_refit_native_partitions.json");
    let sdk = read("observations/sweep1_refit_sdk.json");
    let holdouts = read("observations/sweep1_refit_sdk_holdouts.json");
    let cases = cuts["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    for case in cases {
        let profile = sources["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == case["id"])
            .unwrap();
        let source = super::tests::curve(&profile["curve"]);
        let before = source.clone();
        let tolerance = profile["tolerance"].as_f64().unwrap();
        let row = sdk["results"]
            .as_array()
            .unwrap()
            .iter()
            .chain(holdouts["results"].as_array().unwrap())
            .find(|r| r["id"] == case["id"])
            .unwrap();
        let definition = &row["value"][0]["definition"];
        let nu = definition["control_count"][0].as_u64().unwrap() as usize;
        let native = super::tests::curve(&serde_json::json!({
            "degree": definition["degree"][1], "knots": definition["knots_v"],
            "control_points": definition["control_points"].as_array().unwrap()
                .iter().step_by(nu).collect::<Vec<_>>(),
        }));
        let native_knots = native
            .knots()
            .chunk_by(|a, b| a == b)
            .map(|g| g[0])
            .collect::<Vec<_>>();
        let cuts: Vec<f64> = serde_json::from_value(case["cuts"].clone()).unwrap();
        assert_eq!(cuts.len(), native_knots.len());
        assert_eq!((cuts[0], *cuts.last().unwrap()), (0., 1.));
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT).unwrap();
        let fractions = std::array::from_fn::<_, 9, _>(|i| i as f64 / 8.);
        let lookup = sampler
            .parameters_at_incremental_normalized_lengths(
                &fractions,
                crate::curve_fit::g1::FITTING_BATCH_TOLERANCE,
            )
            .unwrap();
        let parameter = |fraction: f64| {
            let station = fraction * 8.;
            let index = (station.floor() as usize).min(7);
            let blend = station - index as f64;
            lookup[index].mul_add(1. - blend, lookup[index + 1] * blend)
        };
        let mut controls = Vec::new();
        let mut knots = vec![0.; 4];
        for pair in cuts.windows(2) {
            let parameters = fractions.map(|f| parameter(pair[0] + f * (pair[1] - pair[0])));
            let (start, end) = (parameters[0], parameters[8]);
            let initial = initial_cubic_from_frames(
                source.evaluate(start).unwrap(),
                source.evaluate(end).unwrap(),
                source
                    .tangent_at_on_side(start, ParameterSide::Right)
                    .unwrap(),
                source
                    .tangent_at_on_side(
                        end,
                        if end == *source.domain().end() {
                            ParameterSide::Left
                        } else {
                            ParameterSide::Right
                        },
                    )
                    .unwrap(),
            )
            .unwrap();
            let trimmed = source.try_trimmed(start..=end).unwrap();
            let candidate = if within_deviation(&trimmed, &initial, tolerance).unwrap() {
                initial
            } else {
                let adjusted =
                    adjusted_cubic_at_point(&initial, source.evaluate(parameters[4]).unwrap(), 0.)
                        .unwrap()
                        .unwrap();
                assert!(within_deviation(&trimmed, &adjusted, tolerance).unwrap());
                adjusted
            };
            controls.extend(
                candidate
                    .control_points()
                    .iter()
                    .copied()
                    .skip(usize::from(!controls.is_empty())),
            );
            knots.extend([pair[1]; 3]);
        }
        knots.push(1.);
        let assembled = NurbsCurve::try_new_rational(3, controls, knots).unwrap();
        let domain = native.domain();
        let pairs = cuts
            .into_iter()
            .zip(native_knots)
            .map(|(a, b)| [a, (b - domain.start()) / (domain.end() - domain.start())])
            .collect::<Vec<_>>();
        let bound = assembled
            .mapped_curve_deviation_bound(&native, &pairs, 1e-7)
            .unwrap()
            .expect("complete independently constructed boundary must match");
        println!(
            "SWEEP_FIXED_PARTITION_RESULT {}",
            serde_json::json!({
                "id":case["id"],"complete_image_bound":bound,"epsilon":1e-7,
                "pieces":pairs.len()-1,
            })
        );
        assert_eq!(source, before);
    }
}
