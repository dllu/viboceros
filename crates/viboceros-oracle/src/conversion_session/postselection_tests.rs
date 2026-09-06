use super::*;

const CASES: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/nurbs_postselection.json");
const SESSIONS: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/nurbs_postselection_sessions.json");

#[test]
fn permanent_postselection_records_preserve_identity_attributes_and_ordered_memberships() {
    let request: ProbeRequest = serde_json::from_str(CASES).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 87);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::NurbsConversion { fixture, .. } = operation else {
            panic!()
        };
        let before = result.value["before"]["objects"].as_array().unwrap();
        let after = result.value["after"]["objects"].as_array().unwrap();
        assert_eq!(
            after.iter().filter(|o| !o["original"].is_null()).count(),
            before.len()
        );
        assert_eq!(result.value["after"]["groups"].as_array().unwrap().len(), 3);
        if fixture.geometry.postselect {
            assert!(after.iter().all(|o| o["selected"] == false));
        }
        for actual in after {
            let source = if let Some(index) = actual["original"].as_u64() {
                &before[index as usize]
            } else {
                before.iter().find(|o| o["name"] == actual["name"]).unwrap()
            };
            for field in ["name", "color", "color_source", "layer", "groups"] {
                assert_eq!(actual[field], source[field], "{} {field}", result.id);
            }
        }
        if fixture.geometry.cancel && !fixture.geometry.postselect {
            assert_eq!(result.value["before"], result.value["after"]);
        }
    }
}

#[test]
fn postselection_sessions_commit_options_only_after_real_uncancelled_conversions() {
    let request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 3);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::ConversionSession { fixture, .. } = operation else {
            panic!()
        };
        let (mut delete, mut trim) = (None, None);
        for (step, state) in fixture
            .steps
            .iter()
            .zip(result.value["states"].as_array().unwrap())
        {
            if step.command != ConversionCommand::ToNURBS {
                continue;
            }
            let converted = if step.conversion.geometry.cancel {
                0
            } else {
                step.conversion
                    .selected_sources()
                    .filter(|s| converts_to_nurbs(s))
                    .count()
            };
            if converted > 0 {
                delete = step.conversion.geometry.delete_input.or(delete);
                trim = step.conversion.trim_triangular_faces.or(trim);
            }
            let after = state["after"]["objects"].as_array().unwrap();
            assert_eq!(
                after.len(),
                step.conversion.geometry.sources.len()
                    + if delete.unwrap() { 0 } else { converted }
            );
            if step.conversion.geometry.postselect {
                assert!(after.iter().all(|o| o["selected"] == false));
            }
            for brep in after.iter().filter(|o| o["kind"] == "brep") {
                assert_eq!(
                    brep["definition"]["topology"]["faces"][0]["loops"][0]["trims"]
                        .as_array()
                        .unwrap()
                        .len(),
                    if trim.unwrap() { 3 } else { 4 }
                );
            }
        }
    }
}

#[test]
fn cancellation_stages_and_session_seeds_are_validated_before_execution() {
    let mut request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let Operation::ConversionSession { fixture, .. } = &mut request.operations[0] else {
        panic!()
    };
    fixture.steps[0].conversion.geometry.cancel = true;
    assert!(run(fixture, Tolerance::DEFAULT).is_err());
    let value: Value = serde_json::from_str(CASES).unwrap();
    for changes in [
        json!({"cancel_at_selection":true}),
        json!({"cancel":true,"selected":[]}),
        json!({"initial_selection":[0]}),
        json!({"cancel":true,"postselect":false,"cancel_at_selection":true}),
    ] {
        let mut operation = value["operations"][0].clone();
        operation
            .as_object_mut()
            .unwrap()
            .extend(changes.as_object().unwrap().clone());
        let Operation::NurbsConversion { fixture, .. } = serde_json::from_value(operation).unwrap()
        else {
            panic!()
        };
        assert!(run_nurbs(&fixture, Tolerance::DEFAULT).is_err());
    }
}
