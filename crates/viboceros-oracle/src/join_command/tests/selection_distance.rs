use super::*;

#[test]
fn selection_distances_replay_raw_fields_and_keep_endpoint_only_differences_visible() {
    const REQUEST: &str =
        include_str!("../../../../../tools/rhino_oracle/fixtures/join_surface_thresholds.json");
    let (actual, expected) = surfaces::replay(
        REQUEST,
        include_str!("../../../../../tools/rhino_oracle/observations/join_surface_thresholds.json"),
        90,
    );
    let requests: Value = serde_json::from_str(REQUEST).unwrap();
    let differences = [
        "threshold-3-0.0021-post",
        "prethreshold-0-0.001-1.8",
        "prethreshold-3-0.001-1.8",
        "prethreshold-0-0.0001-1.8",
        "prethreshold-0-0.01-1.8",
        "cutoff-0-exact-pre-Join",
        "cutoff-0-exact-pre-JoinCopy",
    ];
    let mut matched = 0;
    for (i, (a, b)) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
        .enumerate()
    {
        let b = &b["value"];
        if !differences.contains(&a.id.as_str()) {
            compare(&a.value, b, &a.id);
            matched += 1;
            continue;
        }
        let (native, rhino) = (surfaces::outputs(&a.value), surfaces::outputs(b));
        assert_eq!(a.value["succeeded"], true, "{}", a.id);
        assert_eq!(b["succeeded"], true, "{}", a.id);
        assert_eq!(rhino.len(), 2);
        for output in &rhino {
            assert_eq!(output["brep"]["faces"].as_array().unwrap().len(), 1);
            assert!(
                output["brep"]["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|e| e["uses"] == 1)
            );
        }
        // Both engines retain the underlying source surfaces. Rhino moves
        // just the top corner pair; neither reference sheet becomes mated.
        let surfaces = |outputs: &[&Value]| {
            outputs
                .iter()
                .flat_map(|o| o["brep"]["surfaces"].as_array().unwrap())
                .cloned()
                .collect::<Vec<_>>()
        };
        compare(&json!(surfaces(&native)), &json!(surfaces(&rhino)), &a.id);
        let sources = &requests["operations"][i]["sources"];
        let left = sources[0]["brep"]["source"]["max"][1].as_f64().unwrap();
        let right = sources[1]["brep"]["source"]["min"][1].as_f64().unwrap();
        let midpoint = (left + right) * 0.5;
        compare(
            &rhino[0]["brep"]["vertices"][3],
            &json!([0., midpoint, 5.]),
            &a.id,
        );
        compare(
            &rhino[1]["brep"]["vertices"][0],
            &json!([0., midpoint, 5.]),
            &a.id,
        );
        compare(
            &rhino[0]["brep"]["vertices"][1],
            &json!([0., left, 0.]),
            &a.id,
        );
        compare(
            &rhino[1]["brep"]["vertices"][1],
            &json!([0., right, 0.]),
            &a.id,
        );
        let originals = |v: &Value| {
            v["objects"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| !o["source"].is_null())
                .cloned()
                .collect::<Vec<_>>()
        };
        compare(&json!(originals(&a.value)), &json!(originals(b)), &a.id);
    }
    assert_eq!(matched, 83);
}

#[test]
fn ordered_cluster_discovery_keeps_successes_mismatches_and_guard_rejections() {
    let mut request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/join_cluster_order.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/join_cluster_order.json"
    ))
    .unwrap();
    let operations = std::mem::take(&mut request.operations);
    assert_eq!(operations.len(), 60);
    assert_eq!(expected["results"].as_array().unwrap().len(), 60);
    let (mut matched, mut rejected, mut mismatched) = (0, 0, 0);
    for (op, record) in operations
        .into_iter()
        .zip(expected["results"].as_array().unwrap())
    {
        request.operations = vec![op];
        let id = record["id"].as_str().unwrap();
        let actual = run_request(&request);
        if id.contains("chain4-") || id.contains("chain5-") {
            let error = actual.unwrap_err().to_string();
            assert!(
                error.contains("adjusted boundary cluster exceeds the join distance"),
                "{id}: {error}"
            );
            assert_eq!(record["value"]["succeeded"], true, "{id}");
            rejected += 1;
            continue;
        }
        let actual = actual.unwrap();
        assert_eq!(actual.results[0].id, id);
        let native = &actual.results[0].value;
        let rhino = &record["value"];
        if id.ends_with("-post") {
            compare(native, rhino, id);
            matched += 1;
        } else {
            let (n, r) = (surfaces::outputs(native), surfaces::outputs(rhino));
            // Explicitly track unresolved topology, not a comparison with
            // stripped geometry or normalized face/edge tables.
            assert_eq!(
                (n.len(), r.len()),
                if id.starts_with("star-") {
                    (5, 4)
                } else {
                    (3, 2)
                },
                "{id}"
            );
            assert!(
                n.iter()
                    .all(|o| o["brep"]["faces"].as_array().unwrap().len() == 1)
            );
            assert_eq!(
                r.iter()
                    .filter(|o| o["brep"]["faces"].as_array().unwrap().len() == 2)
                    .count(),
                1,
                "{id}"
            );
            assert_eq!(native["succeeded"], true);
            assert_eq!(rhino["succeeded"], true);
            mismatched += 1;
        }
    }
    assert_eq!((matched, rejected, mismatched), (12, 20, 28));
}
