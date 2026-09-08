use super::*;

const CASES: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/bezier_postselection.json");
const SESSIONS: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/bezier_postselection_sessions.json");

#[test]
fn bezier_picking_records_fresh_outputs_and_preserves_source_attributes_and_groups() {
    let request: ProbeRequest = serde_json::from_str(CASES).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 61);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::BezierConversion { fixture, .. } = operation else {
            panic!()
        };
        let before = result.value["before"]["objects"].as_array().unwrap();
        let after = result.value["after"]["objects"].as_array().unwrap();
        assert_eq!(result.value["after"]["groups"].as_array().unwrap().len(), 3);
        for object in after {
            if fixture.postselect {
                assert_eq!(object["selected"], false);
            }
            if let Some(index) = object["original"].as_u64() {
                for field in [
                    "name",
                    "layer",
                    "color",
                    "color_source",
                    "groups",
                    "points",
                    "domain",
                ] {
                    assert_eq!(
                        object[field], before[index as usize][field],
                        "{} {field}",
                        result.id
                    );
                }
            } else {
                assert!(!fixture.cancel);
                assert!(object["name"].is_null());
                assert_eq!(object["layer"], "Current");
                assert_eq!(object["groups"], json!([]));
                assert_eq!(object["color"], json!([0, 0, 0]));
                assert_eq!(object["color_source"], "ColorFromLayer");
                assert_eq!(object["selected"], false);
            }
        }
        if fixture.cancel {
            assert_eq!(before.len(), after.len());
            if !fixture.postselect {
                assert_eq!(result.value["before"], result.value["after"]);
            }
        }
    }
}

#[test]
fn bezier_question_memory_changes_only_after_an_uncancelled_answer_and_survives_undo() {
    let request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 2);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::ConversionSession { fixture, .. } = operation else {
            panic!()
        };
        let mut delete = None;
        assert_eq!(fixture.steps.len(), 12);
        for (step, state) in fixture
            .steps
            .iter()
            .zip(result.value["states"].as_array().unwrap())
        {
            let f = &step.conversion.geometry;
            if !f.cancel {
                delete = f.delete_input.or(delete);
            }
            let after = state["after"]["objects"].as_array().unwrap();
            let sources = after.iter().filter(|o| !o["original"].is_null()).count();
            assert_eq!(sources, usize::from(f.cancel || !delete.unwrap()));
            assert_eq!(after.len() - sources, if f.cancel { 0 } else { 2 });
            if f.postselect {
                assert!(after.iter().all(|o| o["selected"] == false));
            }
        }
    }
}

#[test]
fn bezier_prompt_fixtures_reject_impossible_stages_and_cancelled_session_seeds() {
    let value: Value = serde_json::from_str(CASES).unwrap();
    for changes in [
        json!({"cancel_at_selection":true}),
        json!({"cancel":true,"selected":[]}),
        json!({"initial_selection":[0]}),
        json!({"cancel":true,"postselect":false,"cancel_at_selection":true}),
        json!({"cancel":true,"postselect":false,"sources":[{"type":"point","point":[0,0,0]}]}),
    ] {
        let mut operation = value["operations"][0].clone();
        operation
            .as_object_mut()
            .unwrap()
            .extend(changes.as_object().unwrap().clone());
        let Operation::BezierConversion { fixture, .. } =
            serde_json::from_value(operation).unwrap()
        else {
            panic!()
        };
        assert!(crate::conversion::run(&fixture, Tolerance::DEFAULT).is_err());
    }
    let mut request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let Operation::ConversionSession { fixture, .. } = &mut request.operations[0] else {
        panic!()
    };
    fixture.steps[0].conversion.geometry.cancel = true;
    assert!(run(fixture, Tolerance::DEFAULT).is_err());
}
