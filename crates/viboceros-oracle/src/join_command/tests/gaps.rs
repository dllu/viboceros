use super::*;

#[test]
fn gap_rebuilding_replays_every_raw_geometry_tolerance_and_document_field() {
    let (actual, expected) = surfaces::replay(
        include_str!("../../../../../tools/rhino_oracle/fixtures/join_gap_matching.json"),
        include_str!("../../../../../tools/rhino_oracle/observations/join_gap_matching.json"),
        88,
    );
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        compare(&a.value, &b["value"], &a.id);
    }
}

#[test]
fn transitive_chain_matches_command_first_but_batch_adjustment_is_rejected() {
    let mut request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/join_gap_chains.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/join_gap_chains.json"
    ))
    .unwrap();
    let operations = std::mem::take(&mut request.operations);
    assert_eq!(operations.len(), 4);
    for (op, b) in operations
        .into_iter()
        .zip(expected["results"].as_array().unwrap())
    {
        request.operations = vec![op];
        let id = b["id"].as_str().unwrap();
        if id.ends_with("-post") {
            let actual = run_request(&request).unwrap();
            assert_eq!(actual.results[0].id, id);
            compare(&actual.results[0].value, &b["value"], id);
        } else {
            let error = run_request(&request).unwrap_err().to_string();
            assert!(
                error.contains("adjusted boundary cluster exceeds the join distance"),
                "{id}: {error}"
            );
            assert_eq!(b["value"]["succeeded"], true);
            assert_eq!(
                b["value"]["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|o| o["source"].is_null())
                    .count(),
                2
            );
        }
    }
}

#[test]
fn chord_projection_matches_raw_geometry_with_independent_area_witnesses() {
    let (actual, expected) = surfaces::replay(
        include_str!("../../../../../tools/rhino_oracle/fixtures/join_chord_adjustment.json"),
        include_str!("../../../../../tools/rhino_oracle/observations/join_chord_adjustment.json"),
        16,
    );
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        let mut native = a.value.clone();
        let reference = &b["value"];
        let witness = if a.id.starts_with("chord-3-0.5-") {
            Some((0, 12.280_401_451_679_074))
        } else if a.id.starts_with("chord-4-1-") {
            Some((1, 20.000_001_229_541_06))
        } else {
            None
        };
        let mut differences = 0;
        if let Some((source, area)) = witness {
            for (n, r) in native["objects"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(reference["objects"].as_array().unwrap())
            {
                let face = if n["source"].is_null() {
                    source
                } else if n["source"] == json!(source) {
                    0
                } else {
                    continue;
                };
                let actual = n["brep"]["faces"][face][1].as_f64().unwrap();
                let rhino = r["brep"]["faces"][face][1].as_f64().unwrap();
                assert!((actual - area).abs() < 2e-12, "{}: {actual}", a.id);
                assert!(
                    (rhino - area).abs() > 2e-10 && (rhino - area).abs() < 5e-10,
                    "{}: {rhino}",
                    a.id
                );
                // Only this asserted integral is substituted for the full
                // remaining-field comparison. Saved records are always raw.
                n["brep"]["faces"][face][1] = json!(rhino);
                differences += 1;
            }
            assert_eq!(differences, if a.id.contains("JoinCopy") { 2 } else { 1 });
        }
        compare(&native, reference, &a.id);
    }
}
