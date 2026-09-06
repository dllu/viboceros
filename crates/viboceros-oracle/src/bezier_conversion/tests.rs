use super::*;

#[test]
fn permanent_conversion_records_unit_bezier_nets_and_fresh_document_attributes() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/bezier_conversion.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 44);
    for result in response.results {
        let before = &result.value["before"];
        let after = &result.value["after"];
        assert_eq!(before["groups"].as_array().unwrap().len(), 3);
        assert_eq!(after["groups"].as_array().unwrap().len(), 3);
        let objects = after["objects"].as_array().unwrap();
        let mut order = after["creation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        order.sort_unstable();
        assert_eq!(order, (0..objects.len()).collect::<Vec<_>>());
        for object in objects.iter().filter(|o| o["original"].is_null()) {
            assert!(object["name"].is_null());
            assert_eq!(object["layer"], "Current");
            assert_eq!(object["color"], json!([0, 0, 0]));
            assert_eq!(object["color_source"], "ColorFromLayer");
            assert_eq!(object["selected"], false);
            assert_eq!(object["groups"], json!([]));
            let def = &object["definition"];
            if object["kind"] == "curve" {
                assert_eq!(object["domain"], json!([0., 1.]));
                let p = def["degree"].as_u64().unwrap() as usize;
                assert_eq!(def["control_points"].as_array().unwrap().len(), p + 1);
                assert_eq!(
                    def["knots"],
                    json!(
                        (0..2 * (p + 1))
                            .map(|i| if i <= p { 0. } else { 1. })
                            .collect::<Vec<_>>()
                    )
                );
            } else {
                assert_eq!(object["kind"], "surface");
                assert_eq!(object["domain"], json!([[0., 1.], [0., 1.]]));
                for axis in 0..2 {
                    assert_eq!(
                        def["control_count"][axis].as_u64().unwrap(),
                        def["degree"][axis].as_u64().unwrap() + 1
                    );
                }
            }
        }
    }
}

#[test]
fn fixture_validation_rejects_empty_duplicate_and_out_of_range_selection() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/bezier_conversion.json"
    ))
    .unwrap();
    let Operation::BezierConversion { fixture, .. } = &request.operations[0] else {
        panic!()
    };
    for selected in [vec![], vec![0, 0], vec![1]] {
        let mut f = fixture.clone();
        f.selected = Some(selected);
        assert!(run(&f, Tolerance::DEFAULT).is_err());
    }
    let mut f = fixture.clone();
    f.sources.clear();
    assert!(run(&f, Tolerance::DEFAULT).is_err());
}
