use super::*;

#[test]
fn stationary_trim_sources_replay_complete_shared_rhino_geometry() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/stationary_trims.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/stationary_trims.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let expected = observed["results"].as_array().unwrap();
    assert_eq!(actual.results.len(), 18);
    assert_eq!(expected.len(), 18);
    for (actual, expected) in actual.results.iter().zip(expected) {
        assert_eq!(expected["id"], actual.id);
        assert!(
            crate::brep_interchange::roundtrip_equal(&actual.value, &expected["value"]),
            "{}",
            actual.id
        );
    }
}

#[test]
fn solid_orientation_shared_sources_resolve_corner_cases_but_retain_coincident_gaps() {
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
    assert_eq!(complete_matches, 47);
    assert_eq!(
        unresolved,
        ["coincident-opposed", "coincident-opposed-reverse-order"]
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

#[test]
fn solid_orientation_polyhedra_keep_exact_geometry_and_expose_rhino_translation_differences() {
    let input: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/solid_orientation_polyhedra.json"
    ))
    .unwrap();
    let request: ProbeRequest = serde_json::from_value(input.clone()).unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/solid_orientation_polyhedra.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let expected = observed["results"].as_array().unwrap();
    assert_eq!(actual.results.len(), 68);
    assert_eq!(expected.len(), 68);
    let mut matches = 0;
    for ((actual, expected), source) in actual
        .results
        .iter()
        .zip(expected)
        .zip(input["operations"].as_array().unwrap())
    {
        assert_eq!(expected["id"], actual.id);
        assert_eq!(source["id"], actual.id);
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
        let sources = source["sources"].as_array().unwrap();
        // Input-only analytic witnesses: determinant +42 (or -42 for the
        // reflection); the small box is the unique leftmost compound shell.
        let inward = if sources.len() == 1 {
            sources[0]["reversed"].as_bool().unwrap() ^ actual.id.contains("-reflection-")
        } else {
            sources
                .iter()
                .find(|s| s["source"]["type"] == "box")
                .unwrap()["reversed"]
                .as_bool()
                .unwrap()
        };
        assert_eq!(
            actual.value["orientation"],
            if inward { "Inward" } else { "Outward" },
            "{}",
            actual.id
        );
        let equal = actual.value["orientation"] == expected["value"]["orientation"];
        let recorded_translation_difference = actual.id.contains("-translated-")
            && (actual.id.starts_with("tetrahedron-")
                || actual.id.starts_with("square_tube-")
                || actual.id.starts_with("box-") && actual.id.ends_with("order-True"));
        assert_eq!(equal, !recorded_translation_difference, "{}", actual.id);
        matches += usize::from(equal);
    }
    assert_eq!(matches, 58);
}
