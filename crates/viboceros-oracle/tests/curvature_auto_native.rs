//! Public SDK statistics on synthetic cached curvature channels, independent
//! of either product's surface tessellator. Native rounding artifacts remain
//! in the observations; the exceptions below assert our stable behavior.
use serde_json::Value;
use std::{collections::BTreeSet, path::Path};
use viboceros_geometry::CurvatureStyle;

#[test]
fn automatic_ranges_match_native_sdk_except_recorded_rounding_artifacts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read =
        |path| serde_json::from_slice::<Value>(&std::fs::read(root.join(path)).unwrap()).unwrap();
    let query = read("fixtures/curvature_auto_sdk.json");
    let samples = read("fixtures/curvature_auto/samples.json");
    let native: Value = serde_json::from_reader(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("observations/curvature_auto_sdk.json.gz")).unwrap(),
    ))
    .unwrap();
    let mut artifacts = BTreeSet::new();
    let expected_artifacts = [
        "weighted-MaxRadius",
        "split103-Gaussian",
        "split103-Mean",
        "split103-MinRadius",
        "split103-MaxRadius",
        "duplicate103-Gaussian",
        "duplicate103-Mean",
        "duplicate103-MinRadius",
        "duplicate103-MaxRadius",
        "spread_small_8-Mean",
    ]
    .map(String::from)
    .into_iter()
    .collect::<BTreeSet<_>>();
    let ops = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(ops.len(), 536);
    assert_eq!(rows.len(), ops.len());
    for (op, row) in ops.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        assert!(row.get("error").is_none());
        let index = CurvatureStyle::ALL
            .iter()
            .position(|s| s.name() == op["style"].as_str().unwrap())
            .unwrap();
        let style = CurvatureStyle::ALL[index];
        let assets = op["asset"].as_str().into_iter().chain(
            op["companions"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|s| s.as_str().unwrap()),
        );
        let values = assets
            .flat_map(|asset| {
                samples[asset]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| serde_json::from_value::<[f64; 2]>(k.clone()).unwrap())
            })
            .collect::<Vec<_>>();
        assert_eq!(row["value"]["count"], values.len());
        let actual = style.automatic_range(values.clone());
        if values.is_empty() {
            assert_eq!(actual, None);
            assert_eq!(row["value"]["success"], false);
            continue;
        }
        assert_eq!(row["value"]["success"], true);
        let actual = actual.unwrap();
        let expected: [f64; 2] =
            serde_json::from_value(row["value"]["ranges"][index].clone()).unwrap();
        let scale = actual
            .into_iter()
            .chain(expected)
            .map(f64::abs)
            .fold(1e-300, f64::max);
        if actual
            .into_iter()
            .zip(expected)
            .any(|(a, b)| (a - b).abs() > 1e-12 * scale)
        {
            let id = op["id"].as_str().unwrap();
            assert!(
                expected_artifacts.contains(id),
                "{id}: {actual:?} vs {expected:?}"
            );
            artifacts.insert(id.to_owned());
            if id == "spread_small_8-Mean" {
                assert!(actual[1] - actual[0] < 0.001);
                assert_eq!(expected, [0.500000005, 1.00000001]);
            } else {
                assert_eq!(actual, [1., 1.]);
                assert!(values.iter().all(|k| style.value(*k) == Some(1.)));
                assert_eq!(
                    expected,
                    if style == CurvatureStyle::Gaussian {
                        [0.9898999999999999, 1.0101]
                    } else {
                        [1., 2.]
                    }
                );
            }
        }
    }
    assert_eq!(artifacts, expected_artifacts);
}
