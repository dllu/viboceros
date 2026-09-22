use super::*;

#[test]
fn solid_orientation_shared_sources_retain_four_explicit_compatibility_gaps() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/solid_orientation.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/solid_orientation.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let expected = observed["results"].as_array().unwrap();
    assert_eq!(actual.results.len(), 49);
    assert_eq!(expected.len(), actual.results.len());
    let mut complete_matches = 0;
    let mut unresolved = Vec::new();
    for (actual, expected) in actual.results.iter().zip(expected) {
        assert_eq!(expected["id"], actual.id);
        for key in ["geometry", "solid", "closed"] {
            assert!(
                crate::brep_interchange::roundtrip_equal(
                    &actual.value[key],
                    &expected["value"][key]
                ),
                "{}: {key}",
                actual.id
            );
        }
        if actual.value["orientation"] == expected["value"]["orientation"] {
            complete_matches += 1;
        } else {
            assert_eq!(actual.value["orientation"], "Unknown", "{}", actual.id);
            assert!(matches!(
                expected["value"]["orientation"].as_str(),
                Some("Inward" | "Outward")
            ));
            unresolved.push(actual.id.as_str());
        }
    }
    assert_eq!(complete_matches, 45);
    assert_eq!(
        unresolved,
        [
            "coincident-opposed",
            "coincident-opposed-reverse-order",
            "corner-False",
            "corner-True"
        ]
    );
}

#[test]
fn solid_orientation_rejects_invalid_source_counts_iterations_and_face_indices() {
    let source = json!({"source":{"type":"box","min":[-1,-1,-1],"max":[1,1,1]}});
    for (sources, flip_faces, iterations) in [
        (vec![], vec![], 1),
        (vec![source.clone(); 9], vec![], 1),
        (vec![source.clone()], vec![], 0),
        (vec![source.clone()], vec![], 2),
        (vec![source.clone()], vec![0, 0], 1),
        (vec![source], vec![6], 1),
    ] {
        let fixture =
            serde_json::from_value(json!({"sources":sources,"flip_faces":flip_faces})).unwrap();
        assert!(matches!(
            run(&fixture, iterations, Tolerance::DEFAULT),
            Err(ProbeError::FixtureInvariant(_))
        ));
    }
}
