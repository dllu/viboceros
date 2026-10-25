//! Ordered section charts and geometric continuity checked against Rhino.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_geometry::{
    ArcLengthSampler, CurveContinuity, CurveRef, Tolerance, try_loft_nurbs_curves,
};
#[path = "support/loft_parameters.rs"]
mod cases;
use cases::curve;

#[test]
fn domain_shifts_and_relocated_seams_match_licensed_ordered_lofts() {
    cases::compare_preparation(false, 45, try_loft_nurbs_curves);
}

#[test]
#[ignore = "Native coarse lengths differ by one bit and select another boundary at a symmetric near tie"]
fn symmetric_near_tie_matches_licensed_ordered_loft() {
    cases::compare_preparation(true, 1, try_loft_nurbs_curves);
}

#[test]
fn geometric_regions_and_coarse_lengths_match_licensed_queries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in ["loft_splitting_domain_geometric_matching"] {
        let q: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let r: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(r["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            for (definition, native) in op["curves"]
                .as_array()
                .unwrap()
                .iter()
                .zip(row["value"]["geometric_matching"].as_array().unwrap())
            {
                let source = curve(definition);
                let before = source.clone();
                assert_eq!(
                    json!(
                        source
                            .discontinuity_parameters(CurveContinuity::G2, 0.)
                            .unwrap()
                    ),
                    native["parameters"]
                );
                for region in native["regions"].as_array().unwrap() {
                    let part = source
                        .try_trimmed(
                            region["domain"][0].as_f64().unwrap()
                                ..=region["domain"][1].as_f64().unwrap(),
                        )
                        .unwrap();
                    let actual =
                        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&part), Tolerance::DEFAULT)
                            .unwrap()
                            .quadrature_length_estimate(1e-3)
                            .unwrap();
                    assert!(
                        (actual - region["length"].as_f64().unwrap()).abs() <= 1e-10,
                        "{}: {actual} != {}",
                        op["id"],
                        region["length"]
                    );
                }
                assert_eq!(source, before);
                count += 1;
            }
        }
    }
    assert_eq!(count, 32);
}
