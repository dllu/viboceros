//! Source length batches on independent adaptive-fitting holdouts.
use serde_json::Value;
use viboceros_geometry::{ArcLengthSampler, CurveRef, Tolerance};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn independent_adaptive_sources_retain_the_measured_fitting_clock() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_adaptive_shape_clocks.json")))
                .unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    assert_eq!(query["operations"].as_array().unwrap().len(), 4);
    assert_eq!(native["results"].as_array().unwrap().len(), 4);
    let fractions = std::array::from_fn::<_, 9, _>(|i| i as f64 / 8.);
    let (mut maximum, mut maximum_cut_error, mut cuts) = (0_f64, 0_f64, 0);
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let source = definitions::curve(&op["curves"][0]);
        let before = source.clone();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT).unwrap();
        let parameters = sampler
            .parameters_at_incremental_normalized_lengths(&fractions, 1e-2)
            .unwrap();
        let clock = &row["value"]["refit_sources"][1]["numerical_clocks"][3];
        assert_eq!(clock["fractional_tolerance"], 1e-2);
        let mut local = 0_f64;
        for (i, parameter) in parameters.iter().enumerate() {
            local = local.max((parameter - clock["batch_parameters"][i].as_f64().unwrap()).abs());
        }
        println!(
            "ADAPTIVE_SOURCE_CLOCK {} maximum_parameter_error={local}",
            op["id"]
        );
        assert!(local <= 1e-12, "{}: {local}", op["id"]);
        maximum = maximum.max(local);
        // Native C0 region boundaries retain their original source points.
        // Check that their positions come from dyadic interpolation in the
        // original lookup, independently of Native's later knot retiming.
        let paired = &row["value"]["refit_sources"][1];
        let fitted = definitions::curve(&paired["curves"][0]);
        let dyadic = (1..128)
            .map(|i| {
                let station = f64::from(i) / 16.;
                let index = (station.floor() as usize).min(7);
                let blend = station - index as f64;
                source
                    .evaluate(parameters[index].mul_add(1. - blend, parameters[index + 1] * blend))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        for group in fitted
            .knots()
            .chunk_by(|a, b| a == b)
            .filter(|g| g.len() == 3)
        {
            let point = fitted.evaluate(group[0]).unwrap();
            let error = dyadic
                .iter()
                .map(|p| point.distance_to(*p).unwrap())
                .fold(f64::INFINITY, f64::min);
            assert!(error <= 1e-12, "{}: cut {}: {error}", op["id"], group[0]);
            maximum_cut_error = maximum_cut_error.max(error);
            cuts += 1;
        }
        assert_eq!(source, before);
    }
    assert_eq!(cuts, 26);
    println!("ADAPTIVE_SOURCE_CUTS boundaries={cuts} maximum_point_error={maximum_cut_error}");
    println!("ADAPTIVE_SOURCE_CLOCKS records=4 parameters=36 maximum_parameter_error={maximum}");
}
