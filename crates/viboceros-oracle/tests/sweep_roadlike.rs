use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};

#[test]
fn json_default_and_explicit_axes_reach_both_sweep_builders() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/sweep1_roadlike_explicit.json"
    ))
    .unwrap();
    for command in [false, true] {
        let mut explicit = source.clone();
        explicit["operations"].as_array_mut().unwrap().retain(|op| {
            ["xz-retained", "line-z", "spatial"].contains(&op["id"].as_str().unwrap())
        });
        for op in explicit["operations"].as_array_mut().unwrap() {
            op["command"] = command.into();
        }
        let mut inferred = explicit.clone();
        for op in inferred["operations"].as_array_mut().unwrap() {
            op.as_object_mut().unwrap().remove("roadlike_axis");
        }
        let probe = |value: Value| {
            let request: ProbeRequest = serde_json::from_value(value).unwrap();
            run_request(&request)
        };
        let default = probe(inferred).unwrap();
        let reference = probe(explicit.clone()).unwrap();
        assert_eq!(default.results.len(), 3);
        for (a, b) in default.results.iter().zip(&reference.results) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.value.as_array().unwrap().len(), 1);
            let (a, b) = (&a.value[0]["definition"], &b.value[0]["definition"]);
            assert_eq!(a["control_count"], b["control_count"]);
            assert_eq!(a["knots_u"], b["knots_u"]);
            assert_eq!(a["knots_v"], b["knots_v"]);
            for (a, b) in a["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .zip(b["control_points"].as_array().unwrap())
            {
                assert!(
                    (a["weight"].as_f64().unwrap() - b["weight"].as_f64().unwrap()).abs() < 1e-10
                );
                for i in 0..3 {
                    assert!(
                        (a["point"][i].as_f64().unwrap() - b["point"][i].as_f64().unwrap()).abs()
                            < 1e-10
                    );
                }
            }
        }
        explicit["operations"]
            .as_array_mut()
            .unwrap()
            .retain(|op| op["id"] == "line-z");
        explicit["operations"][0]["roadlike_axis"] = serde_json::json!([0., 0., 1.]);
        assert!(
            probe(explicit).is_err(),
            "explicit axis must override inference"
        );
    }
}
