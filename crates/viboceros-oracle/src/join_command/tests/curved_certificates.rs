use super::*;

fn replay() -> (ProbeResponse, Value) {
    surfaces::replay(
        include_str!("../../../../../tools/rhino_oracle/fixtures/join_curved_certificates.json"),
        include_str!(
            "../../../../../tools/rhino_oracle/observations/join_curved_certificates.json"
        ),
        48,
    )
}

fn discrepancy(id: &str) -> bool {
    id.starts_with("weights-near-") || id.starts_with("unclamped-Join")
}

#[test]
fn independent_curved_bases_replay_complete_rhino_command_records() {
    let (actual, expected) = replay();
    let mut matched = 0;
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        if discrepancy(&a.id) {
            continue;
        }
        compare(&a.value, &b["value"], &a.id);
        matched += 1;
    }
    assert_eq!(matched, 40);
}

#[test]
fn parameter_distance_uncertainty_and_interchange_end_knots_remain_explicit_differences() {
    let (actual, expected) = replay();
    let mut different = 0;
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        if !discrepancy(&a.id) {
            continue;
        }
        let mut compared = a.value.clone();
        let b = &b["value"];
        let mut fields = 0;
        for (object, reference) in compared["objects"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(b["objects"].as_array().unwrap())
        {
            if a.id.starts_with("weights-near-") {
                if !object["source"].is_null() {
                    continue;
                }
                let actual_bound = object["brep"]["edge_tolerances"][0].as_f64().unwrap();
                let reference_bound = reference["brep"]["edge_tolerances"][0].as_f64().unwrap();
                assert!((actual_bound - 0.0004504074551483943).abs() < 1e-14);
                assert!((reference_bound - 0.0003749812509381556).abs() < 1e-14);
                assert!(actual_bound > reference_bound);
                object["brep"]["edge_tolerances"][0] = json!(reference_bound);
                fields += 1;
            } else {
                for (surface, reference) in object["brep"]["surfaces"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .zip(reference["brep"]["surfaces"].as_array().unwrap())
                {
                    if surface["knots_u"] != json!([-2., -1., 0., 1., 2., 3.]) {
                        continue;
                    }
                    // OpenNURBS omits these redundant exterior knot slots.
                    // JoinCopy originals expose the same interchange change.
                    assert_eq!(reference["knots_u"], json!([-1., -1., 0., 1., 2., 2.]));
                    surface["knots_u"] = reference["knots_u"].clone();
                    fields += 1;
                }
            }
        }
        assert_eq!(
            fields,
            if a.id.starts_with("unclamped-JoinCopy") {
                2
            } else {
                1
            }
        );
        // Only after asserting each specific raw discrepancy, verify all other
        // fields. These eight records are not included in the matching count.
        compare(&compared, b, &a.id);
        different += 1;
    }
    assert_eq!(different, 8);
}
