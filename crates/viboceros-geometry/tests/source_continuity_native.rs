//! Retain source-chart derivatives alongside geometric smoothness.
use serde_json::Value;
use viboceros_geometry::{CurveContinuity, CurveParametricContinuity, ParameterSide};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn native_source_continuity_and_limiting_jets_survive_degree_and_chart_changes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let native: Value = serde_json::from_slice(
        &std::fs::read(root.join("observations/loft_refit_source_continuity.json")).unwrap(),
    )
    .unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(rows.len(), 64);
    let (mut queries, mut jets, mut maximum) = (0, 0, 0_f64);
    for row in rows {
        assert_eq!(row["value"]["before"], row["value"]["after"]);
        for source in &row["value"]["refit_sources"].as_array().unwrap()[..4] {
            let info = &source["source_structure"];
            let curve = definitions::curve(&info["nurbs"]);
            let before = curve.clone();
            let actual = [
                (
                    "C1_continuous",
                    curve
                        .parametric_discontinuity_parameters(CurveParametricContinuity::C1)
                        .unwrap(),
                ),
                (
                    "G1_continuous",
                    curve
                        .discontinuity_parameters(CurveContinuity::G1, 1_f64.to_radians())
                        .unwrap(),
                ),
                (
                    "G2_continuous",
                    curve
                        .discontinuity_parameters(CurveContinuity::G2, 1_f64.to_radians())
                        .unwrap(),
                ),
            ];
            for (name, parameters) in actual {
                let expected = info["discontinuities"][name].as_array().unwrap();
                assert_eq!(parameters.len(), expected.len(), "{} {name}", row["id"]);
                for (a, b) in parameters.iter().zip(expected) {
                    let error = (a - b.as_f64().unwrap()).abs();
                    assert!(error <= 1e-9, "{} {name}: {error}", row["id"]);
                    maximum = maximum.max(error);
                }
                queries += 1;
            }
            for span in info["spans"].as_array().unwrap() {
                for (i, side) in [ParameterSide::Right, ParameterSide::Left]
                    .into_iter()
                    .enumerate()
                {
                    let parameter = span["domain"][i].as_f64().unwrap();
                    let derivative = curve
                        .evaluate_with_derivative_on_side(parameter, side)
                        .unwrap()
                        .1
                        .to_array();
                    for (a, b) in derivative
                        .into_iter()
                        .zip(span["derivatives"][i].as_array().unwrap())
                    {
                        let error = (a - b.as_f64().unwrap()).abs();
                        assert!(error <= 1e-9, "{} {parameter}: {error}", row["id"]);
                        maximum = maximum.max(error);
                    }
                    jets += 1;
                }
            }
            assert_eq!(curve, before);
        }
    }
    println!("SOURCE_CONTINUITY records=64 queries={queries} jets={jets} maximum={maximum}");
    assert_eq!((queries, jets), (768, 1024));
}
