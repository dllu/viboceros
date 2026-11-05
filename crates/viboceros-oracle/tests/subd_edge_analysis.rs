use serde_json::Value;
#[path = "support/subd_direction.rs"]
mod comparison;
#[path = "support/observation.rs"]
mod observation;

fn point_error(a: &Value, b: &Value) -> f64 {
    (0..3)
        .map(|i| (a[i].as_f64().unwrap() - b[i].as_f64().unwrap()).abs())
        .fold(0., f64::max)
}

#[test]
fn native_subd_diagnostic_counts_curves_selection_marking_and_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "subd_edge_analysis_discovery",
        "subd_edge_analysis_controls",
        "subd_edge_analysis_capabilities",
    ] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = observation::read(&base, family);
        let actual = serde_json::to_value(
            viboceros_oracle::run_request(&serde_json::from_value(request.clone()).unwrap())
                .unwrap(),
        )
        .unwrap();
        for ((op, a), b) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(actual["results"].as_array().unwrap())
            .zip(native["results"].as_array().unwrap())
        {
            count += 1;
            assert_eq!(a["id"], b["id"]);
            let key = op["id"].as_str().unwrap();
            let (a, b) = (&a["value"], &b["value"]);
            for phase in ["before", "after", "undo", "redo"] {
                comparison::compare(&a[phase], &b[phase], &format!("{key}/{phase}"));
            }
            let sdk = b["sdk"].as_array().unwrap();
            for edge in a["edges"].as_array().unwrap() {
                let index = edge["index"].as_u64().unwrap() as usize;
                let reference = &sdk[index];
                assert_eq!(reference["index"], index);
                let points = edge["points"].as_array().unwrap();
                let expected = if op["flat"] == true {
                    let p = &reference["control"][0];
                    let q = &reference["control"][1];
                    (0..=32)
                        .map(|i| {
                            serde_json::json!(
                                (0..3)
                                    .map(|k| p[k].as_f64().unwrap() * (1. - i as f64 / 32.)
                                        + q[k].as_f64().unwrap() * i as f64 / 32.)
                                    .collect::<Vec<_>>()
                            )
                        })
                        .collect::<Vec<_>>()
                } else {
                    reference["curve"]["points"].as_array().unwrap().clone()
                };
                let forward = points
                    .iter()
                    .zip(&expected)
                    .map(|(a, b)| point_error(a, b))
                    .fold(0., f64::max);
                let reverse = points
                    .iter()
                    .zip(expected.iter().rev())
                    .map(|(a, b)| point_error(a, b))
                    .fold(0., f64::max);
                assert!(
                    forward.min(reverse) < 1e-9,
                    "{key} edge {index}: {forward} / {reverse}"
                );
                assert_eq!(edge["naked"], reference["faces"] == 1);
                assert_eq!(
                    edge["non_manifold"],
                    reference["faces"].as_u64().unwrap() > 2
                );
            }
            if op["command"] == "ShowEdges" {
                let n = a["counts"]["all"].as_u64().unwrap();
                let history = b["history"].as_str().unwrap();
                assert!(
                    history.contains(&if n == 0 {
                        "Found no edges.".into()
                    } else {
                        format!("Found {n} edge")
                    }),
                    "{key}: {history}"
                );
            }
        }
    }
    assert_eq!(count, 54);
}
