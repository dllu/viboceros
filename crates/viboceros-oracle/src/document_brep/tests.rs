use super::*;

#[test]
fn document_brep_probe_uses_independent_replacement_inputs_and_preserves_metadata() {
    let input: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/document_brep.json"
    ))
    .unwrap();
    let request: ProbeRequest = serde_json::from_value(input.clone()).unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 64);
    for (actual, operation) in actual
        .results
        .iter()
        .zip(input["operations"].as_array().unwrap())
    {
        assert_eq!(actual.id, operation["id"]);
        assert_eq!(actual.elapsed_ns, 0);
        let value = &actual.value;
        for key in ["source_unchanged", "replacement_unchanged", "replaced"] {
            assert_eq!(value[key], true, "{}: {key}", actual.id);
        }
        for key in ["inserted", "replacement"] {
            let record = &value[key];
            assert_eq!(record["selected"], operation["selected"]);
            assert_eq!(record["name"], "Source");
            assert_eq!(record["group_count"], 1);
            assert_eq!(record["object_count"], 1);
            assert_eq!(record["current_layer"], true);
            assert_eq!(record["identity_preserved"], true);
        }
        let mut reversed = value["input"]["geometry"].clone();
        for face in reversed["topology"]["faces"].as_array_mut().unwrap() {
            face["reversed"] = json!(!face["reversed"].as_bool().unwrap());
        }
        assert_eq!(value["replacement_input"]["geometry"], reversed);
    }
}

#[test]
fn document_brep_probe_rejects_invalid_iterations_and_insertion_overloads() {
    let input = json!({"sources":[{"source":{"type":"box","min":[-1,-1,-1],"max":[1,1,1]}}]});
    let fixture = serde_json::from_value(input.clone()).unwrap();
    for count in [0, 2] {
        assert!(matches!(
            run(&fixture, count, Tolerance::DEFAULT),
            Err(ProbeError::FixtureInvariant(_))
        ));
    }
    for field in [json!({"insertion":"unknown"}), json!({"selected":1})] {
        let mut invalid = input.clone();
        invalid
            .as_object_mut()
            .unwrap()
            .extend(field.as_object().unwrap().clone());
        assert!(serde_json::from_value::<DocumentBrepFixture>(invalid).is_err());
    }
}

#[test]
fn shared_document_admission_replay_exposes_normalization_and_coincident_query_gaps() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/document_brep.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/document_brep.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let expected = observed["results"].as_array().unwrap();
    assert_eq!((actual.results.len(), expected.len()), (64, 64));
    let (mut matches, mut normalization_gaps, mut coincident_gaps) = (0, 0, 0);
    for (actual, expected) in actual.results.iter().zip(expected) {
        assert_eq!(actual.id, expected["id"]);
        let nonsolid = actual.id.starts_with("open-") || actual.id.starts_with("inconsistent-");
        if nonsolid {
            crate::test_json::close(&actual.value, &expected["value"], &actual.id, 0., 0.);
            matches += 1;
            continue;
        }
        let coincident = actual.id.starts_with("coincident-");
        let reversed = actual.id.contains("-reverse-True-");
        let mut native = actual.value.clone();
        let mut rhino = expected["value"].clone();
        // Assert each known discrepancy before excluding its field from the
        // remainder comparison. Never turn a mismatching record into a match.
        for (path, inward, document_state) in [
            ("/input", reversed, false),
            ("/replacement_input", !reversed, false),
            ("/inserted/geometry", reversed, true),
            ("/replacement/geometry", !reversed, true),
        ] {
            let n = native.pointer_mut(path).unwrap();
            let r = rhino.pointer_mut(path).unwrap();
            let native_sense = if coincident {
                "Unknown"
            } else if inward {
                "Inward"
            } else {
                "Outward"
            };
            let rhino_sense = if document_state || !inward {
                "Outward"
            } else {
                "Inward"
            };
            assert_eq!(
                n.as_object_mut().unwrap().remove("orientation").unwrap(),
                native_sense,
                "{}:{path}",
                actual.id
            );
            assert_eq!(
                r.as_object_mut().unwrap().remove("orientation").unwrap(),
                rhino_sense,
                "{}:{path}",
                actual.id
            );
            if document_state && inward {
                let nf = n["geometry"]["topology"]["faces"].as_array_mut().unwrap();
                let rf = r["geometry"]["topology"]["faces"].as_array_mut().unwrap();
                assert_eq!(nf.len(), rf.len());
                for (n, r) in nf.iter_mut().zip(rf) {
                    let n = n
                        .as_object_mut()
                        .unwrap()
                        .remove("reversed")
                        .unwrap()
                        .as_bool()
                        .unwrap();
                    let r = r
                        .as_object_mut()
                        .unwrap()
                        .remove("reversed")
                        .unwrap()
                        .as_bool()
                        .unwrap();
                    assert_ne!(n, r, "{}:{path}", actual.id);
                }
            }
        }
        // Full raw definitions and metadata otherwise agree exactly, with no
        // gauge/domain/component-order normalization or modeling epsilon.
        crate::test_json::close(&native, &rhino, &actual.id, 0., 0.);
        if coincident {
            coincident_gaps += 1;
        } else {
            normalization_gaps += 1;
        }
    }
    assert_eq!((matches, normalization_gaps, coincident_gaps), (16, 40, 8));
}
