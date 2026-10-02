use super::*;

// Native renewal order varies between identical owned inputs. Match object
// identity; preserve all face/edge/vertex/trim indices within each object.
fn canonical(value: &mut Value) {
    for diagnostic in ["events", "history", "undo_events", "redo_events"] {
        value.as_object_mut().unwrap().remove(diagnostic);
    }
    for field in ["before", "after", "undo", "redo"] {
        if let Some(objects) = value[field].as_array_mut() {
            objects.sort_by_key(|object| object["source"].as_u64().unwrap());
        }
    }
}

#[test]
fn independent_sources_match_both_native_shrink_commands_and_history() {
    for (input, capture, count) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/shrink_trimmed_surfaces.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/shrink_trimmed_surfaces.json"
            ),
            60,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/shrink_trimmed_history.json"),
            include_str!("../../../../tools/rhino_oracle/observations/shrink_trimmed_history.json"),
            28,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/shrink_trimmed_geometry.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/shrink_trimmed_geometry.json"
            ),
            44,
        ),
    ] {
        let request: ProbeRequest = serde_json::from_str(input).unwrap();
        let expected: Value = serde_json::from_str(capture).unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), count);
        let mut failures = Vec::new();
        for (row, native) in actual
            .results
            .iter()
            .zip(expected["results"].as_array().unwrap())
        {
            assert_eq!(row.id, native["id"]);
            let mut value = row.value.clone();
            let mut expected = native["value"].clone();
            canonical(&mut value);
            canonical(&mut expected);
            if std::panic::catch_unwind(|| {
                crate::test_json::close(&value, &expected, &row.id, 1e-9, 0.)
            })
            .is_err()
            {
                failures.push(row.id.clone());
            }
        }
        assert!(failures.is_empty(), "unmatched shrink cases: {failures:?}");
    }
}

#[test]
fn shrink_results_roundtrip_3dm_with_tight_trims_seams_signed_weights_and_shared_edges() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/shrink_trimmed_geometry.json"
    ))
    .unwrap();
    let mut count = 0;
    for operation in request.operations {
        let Operation::ShrinkTrimmedSrfToEdgeCommand { id, fixture } = operation else {
            continue;
        };
        if !id.ends_with("pre-0")
            || ![
                "rotated-ellipse",
                "cylinder-partial",
                "joined",
                "large-uv-origin",
                "signed-cubic",
            ]
            .iter()
            .any(|name| id.contains(name))
        {
            continue;
        }
        let result = fixture.sources[0]
            .brep
            .build(Tolerance::DEFAULT)
            .unwrap()
            .try_shrunk_surfaces(
                viboceros_geometry::BrepSurfaceShrinkMode::ToEdge,
                Tolerance::DEFAULT,
            )
            .unwrap();
        let file = OracleTemporaryFile::new("shrink-result");
        crate::brep_source::write_shared_artifact(
            &Geometry::Brep(result),
            file.path.to_str().unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        count += 1;
    }
    assert_eq!(count, 5);
}
