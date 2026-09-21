use super::*;

#[test]
fn short_overlap_discovery_preserves_complete_edges_and_exposes_partial_trim_limits() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/join_short_overlaps.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/join_short_overlaps.json"
    ))
    .unwrap();
    let native = run_request_audit(&request).unwrap();
    assert_eq!(native.outcomes.len(), 102);
    assert_eq!(expected["results"].as_array().unwrap().len(), 102);
    let (mut matched, mut offsets, mut subdivision, mut order) = (0, 0, 0, 0);
    for (outcome, reference) in native
        .outcomes
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        let id = reference["id"].as_str().unwrap();
        let expected = &reference["value"];
        let result = match outcome {
            OperationOutcome::Success { result } => result,
            OperationOutcome::Failure { id, error } => panic!("{id}: {error:?}"),
        };
        assert_eq!(result.id, id);
        let actual = &result.value;
        if matches!(
            id,
            "offset-crossing-0.0015-0.001-pre"
                | "offset-crossing-0.0015-0.0015-pre"
                | "offset-crossing-0.0015-0.0015-post"
                | "offset-crossing-0.0005-0.00175-pre"
        ) {
            // These are NOT full matches. Rhino preserves original tips where
            // native averages them, and the final case subdivides a different
            // edge. Retain the full records and compare unaffected surfaces.
            assert_eq!(actual["succeeded"], true);
            assert_eq!(surfaces::outputs(actual).len(), 1);
            let a = &surfaces::outputs(actual)[0]["brep"];
            let b = &surfaces::outputs(expected)[0]["brep"];
            for brep in [a, b] {
                let edges = brep["edges"].as_array().unwrap();
                assert_eq!(edges.len(), 9);
                assert_eq!(edges.iter().filter(|e| e["uses"] == 2).count(), 1);
                assert_eq!(brep["vertices"].as_array().unwrap().len(), 8);
            }
            compare(&a["surfaces"], &b["surfaces"], &format!("{id}/surfaces"));
            let (af, bf) = (
                a["faces"].as_array().unwrap(),
                b["faces"].as_array().unwrap(),
            );
            assert_eq!(af.len(), bf.len());
            for (a, b) in af.iter().zip(bf) {
                compare(&a[1], &b[1], &format!("{id}/area"));
            }
            if id != "offset-crossing-0.0005-0.00175-pre" {
                compare(&a["faces"], &b["faces"], &format!("{id}/incidence"));
                compare(
                    &a["face_reversed"],
                    &b["face_reversed"],
                    &format!("{id}/face_reversed"),
                );
            } else {
                // Splitting a different incident side also changes the mating
                // direction and therefore the second face's orientation.
                assert_eq!(a["face_reversed"], json!([false, true]));
                assert_eq!(b["face_reversed"], json!([false, false]));
            }
            offsets += 1;
        } else if id == "offset-crossing-0.0005-0.002-pre" {
            let (a, b) = (surfaces::outputs(actual), surfaces::outputs(expected));
            assert_eq!((a.len(), b.len()), (2, 2));
            assert_eq!(a[0]["brep"]["edges"].as_array().unwrap().len(), 4);
            assert_eq!(b[0]["brep"]["edges"].as_array().unwrap().len(), 5);
            compare(a[1], b[1], id);
            subdivision += 1;
        } else if id == "rotated-crossing-0.0015-True-pre" {
            let mut compared = actual.clone();
            let b = &mut compared["objects"][1]["brep"];
            for key in ["vertices", "vertex_tolerances"] {
                b[key].as_array_mut().unwrap().swap(0, 1);
            }
            for e in b["edges"].as_array_mut().unwrap() {
                for v in e["vertices"].as_array_mut().unwrap() {
                    if v == 0 {
                        *v = json!(1);
                    } else if v == 1 {
                        *v = json!(0);
                    }
                }
            }
            // Only the table remap agrees, not the raw ownership/order record.
            compare(&compared, expected, id);
            order += 1;
        } else {
            compare(actual, expected, id);
            matched += 1;
        }
    }
    assert_eq!((matched, offsets, subdivision, order), (96, 4, 1, 1));
}
