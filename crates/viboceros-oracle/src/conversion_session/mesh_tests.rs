use super::*;

const MESH: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/mesh_nurbs_conversion.json");
const SESSIONS: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/mesh_nurbs_conversion_sessions.json");
const POST: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/mesh_nurbs_postselection.json");
const POST_SESSIONS: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/mesh_nurbs_postselection_sessions.json");

#[test]
fn prompted_mesh_records_clear_picks_without_changing_sources_or_groups() {
    let request: ProbeRequest = serde_json::from_str(POST).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 60);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::MeshNurbsConversion { fixture, .. } = operation else {
            panic!()
        };
        let before = result.value["before"]["objects"].as_array().unwrap();
        let after = result.value["after"]["objects"].as_array().unwrap();
        assert_eq!(
            result.value["before"]["groups"],
            result.value["after"]["groups"]
        );
        for (source, actual) in before.iter().zip(after) {
            let mut expected = source.clone();
            expected["selected"] = json!(false);
            assert_eq!(actual, &expected, "{}", result.id);
        }
        assert!(after.iter().all(|o| o["selected"] == false));
        if fixture.geometry.cancel {
            assert_eq!(after.len(), before.len());
        } else {
            assert!(after.len() > before.len());
            for output in &after[before.len()..] {
                assert_eq!(output["kind"], "brep");
                assert_eq!(output["groups"], json!([]));
                let source = before.iter().find(|s| s["name"] == output["name"]).unwrap();
                for key in ["name", "layer", "color", "color_source"] {
                    assert_eq!(output[key], source[key]);
                }
            }
        }
    }
}

#[test]
fn cancelled_mesh_prompts_accept_options_before_and_after_picks_without_outputs() {
    let request: ProbeRequest = serde_json::from_str(POST_SESSIONS).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 3);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::ConversionSession { fixture, .. } = operation else {
            panic!()
        };
        let mut trim = None;
        for (step, state) in fixture
            .steps
            .iter()
            .zip(result.value["states"].as_array().unwrap())
        {
            if step.command != ConversionCommand::MeshToNURB {
                continue;
            }
            trim = step.conversion.trim_triangular_faces.or(trim);
            let outputs = state["after"]["objects"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["original"].is_null())
                .collect::<Vec<_>>();
            assert_eq!(outputs.len(), usize::from(!step.conversion.geometry.cancel));
            for output in outputs {
                let trims = output["definition"]["topology"]["faces"][0]["loops"][0]["trims"]
                    .as_array()
                    .unwrap();
                assert_eq!(trims.len(), if trim.unwrap() { 3 } else { 4 });
            }
        }
    }
}

#[test]
fn postselection_fixture_validation_rejects_impossible_prompt_paths() {
    let value: Value = serde_json::from_str(POST).unwrap();
    for changes in [
        json!({"cancel":true,"postselect":false}),
        json!({"initial_selection":[0]}),
        json!({"initial_selection":[99]}),
        json!({"selected":[]}),
        json!({"selected":[0,0]}),
    ] {
        let mut operation = value["operations"][0].clone();
        operation
            .as_object_mut()
            .unwrap()
            .extend(changes.as_object().unwrap().clone());
        let operation: Operation = serde_json::from_value(operation).unwrap();
        let Operation::MeshNurbsConversion { fixture, .. } = operation else {
            panic!()
        };
        assert!(run_mesh(&fixture, Tolerance::DEFAULT).is_err());
    }
}

#[test]
fn permanent_mesh_command_fixtures_preserve_sources_and_copy_only_attributes() {
    let request: ProbeRequest = serde_json::from_str(MESH).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 50);
    for result in response.results {
        let before = result.value["before"]["objects"].as_array().unwrap();
        let after = result.value["after"]["objects"].as_array().unwrap();
        assert_eq!(&after[..before.len()], before);
        assert_eq!(
            result.value["before"]["groups"],
            result.value["after"]["groups"]
        );
        assert!(after.len() > before.len());
        for output in &after[before.len()..] {
            assert!(output["original"].is_null());
            assert_eq!(output["kind"], "brep");
            assert_eq!(output["groups"], json!([]));
            assert_eq!(output["selected"], false);
            let source = before.iter().find(|s| s["name"] == output["name"]).unwrap();
            for key in ["name", "layer", "color", "color_source"] {
                assert_eq!(output[key], source[key], "{} {key}", result.id);
            }
            assert!(
                !output["definition"]["topology"]["faces"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn mesh_sessions_remember_triangle_trimming_independently_of_to_nurbs_and_undo() {
    let request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 3);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::ConversionSession { fixture, .. } = operation else {
            panic!()
        };
        let mut trim = None;
        let states = result.value["states"].as_array().unwrap();
        assert_eq!(states.len(), fixture.steps.len());
        for (step, state) in fixture.steps.iter().zip(states) {
            if step.command != ConversionCommand::MeshToNURB {
                continue;
            }
            trim = step.conversion.trim_triangular_faces.or(trim);
            for output in state["after"]["objects"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["original"].is_null())
            {
                for face in output["definition"]["topology"]["faces"]
                    .as_array()
                    .unwrap()
                {
                    let trims = face["loops"][0]["trims"].as_array().unwrap();
                    let triangular =
                        trims.len() == 3 || trims.iter().any(|t| t["type"] == "Singular");
                    if triangular {
                        assert_eq!(trims.len(), if trim.unwrap() { 3 } else { 4 });
                    }
                }
            }
        }
    }
}

#[test]
fn mesh_sessions_and_standalone_probes_reject_invalid_options() {
    let mut request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let Operation::ConversionSession { fixture, .. } = &mut request.operations[0] else {
        panic!()
    };
    fixture.steps[0].conversion.use_ngons = None;
    assert!(run(fixture, Tolerance::DEFAULT).is_err());
    let mut request: ProbeRequest = serde_json::from_str(MESH).unwrap();
    let Operation::MeshNurbsConversion { fixture, .. } = &mut request.operations[0] else {
        panic!()
    };
    fixture.geometry.delete_input = Some(false);
    assert!(run_mesh(fixture, Tolerance::DEFAULT).is_err());
    fixture.geometry.delete_input = None;
    fixture.direction = Some(Direction::U);
    assert!(run_mesh(fixture, Tolerance::DEFAULT).is_err());
    fixture.direction = None;
    assert!(run_nurbs(fixture, Tolerance::DEFAULT).is_err());
}
