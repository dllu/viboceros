//! Native references and certified proposals retain small source features.
use serde_json::Value;
use std::path::Path;
use viboceros_geometry::{
    Tolerance, try_propose_adaptive_linear_refit, try_propose_joined_linear_refit,
};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn native_and_certified_proposals_retain_narrow_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_hidden_features.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    assert_eq!(query["operations"].as_array().unwrap().len(), 16);
    assert_eq!(native["results"].as_array().unwrap().len(), 16);
    let (mut peaks, mut native_ratio, mut proposal_ratio) = (0, 0_f64, 0_f64);
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let source = definitions::curve(&op["curves"][0]);
        let before = source.clone();
        let limit = op["refit_tolerance"].as_f64().unwrap();
        let numerical = Tolerance::try_new(limit * 1e-4, 1e-12, 1e-10).unwrap();
        let native_fit = definitions::curve(&row["value"]["refit_sources"][1]["curves"][0]);
        let raw = try_propose_adaptive_linear_refit(&source, limit)
            .unwrap()
            .unwrap();
        assert!(raw.maximum_deviation_bound <= limit);
        let joined = try_propose_joined_linear_refit(&source, limit, 0.)
            .unwrap()
            .unwrap();
        for point in source
            .control_points()
            .iter()
            .map(|c| c.point())
            .filter(|p| p.y() != 0.)
        {
            let t = native_fit.closest_parameter(point, numerical).unwrap();
            let error = point.distance_to(native_fit.evaluate(t).unwrap()).unwrap();
            assert!(error <= limit, "{} native {error} > {limit}", op["id"]);
            native_ratio = native_ratio.max(error / limit);
            for candidate in [&raw.curve, &joined] {
                let t = candidate.closest_parameter(point, numerical).unwrap();
                let error = point.distance_to(candidate.evaluate(t).unwrap()).unwrap();
                assert!(error <= limit, "{} proposal {error} > {limit}", op["id"]);
                proposal_ratio = proposal_ratio.max(error / limit);
            }
            peaks += 1;
        }
        assert_eq!(source, before);
    }
    assert_eq!(peaks, 24);
    println!(
        "FEATURE_REFINEMENT records=16 peaks={peaks} maximum_native_ratio={native_ratio} maximum_proposal_ratio={proposal_ratio}"
    );
}
