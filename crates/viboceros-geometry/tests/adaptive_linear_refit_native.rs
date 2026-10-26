//! Raw certified piece construction is distinct from Rhino's final joined basis.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_geometry::{NurbsCurve, try_propose_adaptive_linear_refit};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn adaptive_piece_controls_match_native_restrictions_before_joining() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_multispan.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let mut cases = 0;
    let mut pieces = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        if op["curves"][0]["degree"] != 1 {
            continue;
        }
        assert_eq!(op["id"], row["id"]);
        let source = definitions::curve(&op["curves"][0]);
        let before = source.clone();
        let limit = op["refit_tolerance"].as_f64().unwrap();
        let proposal = try_propose_adaptive_linear_refit(&source, limit)
            .unwrap()
            .unwrap();
        assert_eq!(source, before);
        assert!(proposal.maximum_deviation_bound <= limit);
        let surface = &row["value"]["surfaces"][0];
        let stride = surface["control_count"][0].as_u64().unwrap() as usize;
        let native = definitions::curve(
            &json!({"degree":surface["degree"][1],"knots":surface["knots_v"],
            "control_points":surface["control_points"].as_array().unwrap().iter().step_by(stride).collect::<Vec<_>>()}),
        );
        let cuts = native
            .knots()
            .chunk_by(|a, b| a == b)
            .map(|g| g[0])
            .collect::<Vec<_>>();
        assert_eq!(cuts.len(), proposal.arc_length_breaks.len(), "{}", op["id"]);
        for (a, b) in proposal.arc_length_breaks.windows(2).zip(cuts.windows(2)) {
            let actual = proposal.curve.try_trimmed(a[0]..=a[1]).unwrap();
            let expected: NurbsCurve = native.try_trimmed(b[0]..=b[1]).unwrap();
            assert_eq!(actual.control_points().len(), 4);
            assert_eq!(expected.control_points().len(), 4);
            for (a, b) in actual
                .control_points()
                .iter()
                .zip(expected.control_points())
            {
                let error = a.point().distance_to(b.point()).unwrap();
                assert!(error <= 1e-9, "{}: {error}", op["id"]);
                maximum = maximum.max(error);
            }
            pieces += 1;
        }
        cases += 1;
    }
    println!("ADAPTIVE_LINEAR_PIECES cases={cases} pieces={pieces} maximum={maximum}");
    assert_eq!(cases, 30);
}

#[test]
fn fresh_scale_and_uneven_bend_proposals_have_independent_complete_source_bounds() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/loft_refit_adaptive_lines.json");
    let query: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut cases = 0;
    let mut maximum = 0_f64;
    for op in query["operations"].as_array().unwrap() {
        for definition in op["curves"].as_array().unwrap() {
            let source = definitions::curve(definition);
            let before = source.clone();
            let limit = op["refit_tolerance"].as_f64().unwrap();
            let fit = try_propose_adaptive_linear_refit(&source, limit)
                .unwrap()
                .unwrap();
            assert_eq!(source.degree(), 1);
            let mut prefix = vec![0.];
            for pair in source.control_points().windows(2) {
                prefix.push(
                    prefix.last().unwrap() + pair[0].point().distance_to(pair[1].point()).unwrap(),
                );
            }
            let total = *prefix.last().unwrap();
            let pairs = prefix
                .iter()
                .enumerate()
                .map(|(i, d)| [i as f64 / (prefix.len() - 1) as f64, d / total])
                .collect::<Vec<_>>();
            let bound = source
                .mapped_curve_deviation_bound(&fit.curve, &pairs, limit)
                .unwrap()
                .unwrap_or_else(|| panic!("{}", op["id"]));
            assert!(fit.maximum_deviation_bound <= limit);
            maximum = maximum.max(bound / limit);
            cases += 1;
            assert_eq!(source, before);
        }
    }
    println!("ADAPTIVE_LINEAR_BOUNDS cases={cases} maximum_fraction={maximum}");
    assert_eq!(cases, 108);
}
