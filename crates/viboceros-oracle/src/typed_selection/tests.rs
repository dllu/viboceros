use super::*;

#[test]
fn every_selector_scene_retains_its_sources_definitions_layers_groups_and_redo() {
    for command in ["SelExtrusion", "SelBlockInstance", "SelMirroredBlocks"] {
        for scenario in [
            "base",
            "preselected",
            "grouped",
            "hidden",
            "locked",
            "hidden-layer",
            "locked-layer",
            "no-matches",
            "no-matches-preselected",
        ] {
            let (value, _) = run(
                &Fixture {
                    command: command.into(),
                    scenario: scenario.into(),
                },
                1,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(value["unchanged"], true);
            assert_eq!(value["redo_preserved"], true);
            assert_eq!(value["types"].as_array().unwrap().len(), 13);
        }
    }
}

#[test]
fn actual_selection_commands_match_all_native_scenes_without_changing_sources_or_redo() {
    replay("typed_selection", 21);
}

#[test]
fn empty_match_selection_commands_match_native_preselection_behavior() {
    replay("typed_selection_empty", 6);
}

fn replay(family: &str, count: usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let query: ProbeRequest =
        serde_json::from_slice(&fs::read(root.join(format!("fixtures/{family}.json"))).unwrap())
            .unwrap();
    let path = root.join(format!("observations/{family}.json"));
    let native: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let before = query.clone();
    let response = run_request(&query).unwrap();
    assert_eq!(query, before);
    assert_eq!(response.results.len(), count);
    let rows = native["results"].as_array().unwrap();
    assert_eq!(rows.len(), count);
    for (actual, expected) in response.results.iter().zip(rows) {
        assert_eq!(actual.id, expected["id"].as_str().unwrap());
        assert_eq!(actual.value, expected["value"], "{}", actual.id);
        assert_eq!(actual.value["unchanged"], true);
        assert_eq!(actual.value["redo_preserved"], true);
    }
}

#[test]
fn malformed_selector_recipes_fail_before_scene_construction() {
    for (command, scenario, iterations) in [
        ("SelAll", "base", 1),
        ("SelExtrusion", "unknown", 1),
        ("SelExtrusion", "base", 2),
    ] {
        assert!(
            run(
                &Fixture {
                    command: command.into(),
                    scenario: scenario.into()
                },
                iterations,
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
}
