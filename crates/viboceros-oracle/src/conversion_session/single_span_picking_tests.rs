use super::*;

const CASES: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/single_span_postselection.json");
const SESSIONS: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/single_span_postselection_sessions.json");

#[test]
fn single_span_picking_preserves_sources_and_creates_fresh_unselected_surface_objects() {
    let request: ProbeRequest = serde_json::from_str(CASES).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 108);
    for (op, result) in request.operations.iter().zip(response.results) {
        let Operation::SingleSpanConversion { fixture, .. } = op else {
            panic!()
        };
        let f = &fixture.geometry;
        let before = result.value["before"]["objects"].as_array().unwrap();
        let after = result.value["after"]["objects"].as_array().unwrap();
        assert_eq!(result.value["after"]["groups"].as_array().unwrap().len(), 3);
        for object in after {
            if f.postselect {
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
                assert!(!f.cancel);
                assert_eq!(object["kind"], "surface");
                assert!(object["name"].is_null());
                assert_eq!(object["layer"], "Current");
                assert_eq!(object["groups"], json!([]));
                assert_eq!(object["color"], json!([0, 0, 0]));
                assert_eq!(object["color_source"], "ColorFromLayer");
                assert_eq!(object["selected"], false);
            }
        }
        if f.cancel {
            assert_eq!(before.len(), after.len());
        }
        if f.cancel && !f.postselect {
            assert_eq!(result.value["before"], result.value["after"]);
        }
    }
}

#[test]
fn single_span_sessions_accept_cancelled_options_but_not_selection_stage_presets() {
    let request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 3);
    for (op, result) in request.operations.iter().zip(response.results) {
        let Operation::ConversionSession { fixture, .. } = op else {
            panic!()
        };
        let (mut direction, mut delete) = (None, None);
        for (step, state) in fixture
            .steps
            .iter()
            .zip(result.value["states"].as_array().unwrap())
        {
            let f = &step.conversion.geometry;
            if !f.cancel_at_selection {
                direction = step.conversion.direction.or(direction);
                delete = f.delete_input.or(delete);
                if step.conversion.toggles % 2 == 1 {
                    direction = Some(match direction.unwrap() {
                        Direction::U => Direction::V,
                        Direction::V => Direction::U,
                        Direction::Both => panic!(),
                    });
                }
            }
            let after = state["after"]["objects"].as_array().unwrap();
            if f.postselect {
                assert!(after.iter().all(|o| o["selected"] == false));
            }
            let outputs = after
                .iter()
                .filter(|o| o["original"].is_null())
                .collect::<Vec<_>>();
            if f.cancel {
                assert!(outputs.is_empty());
            }
            if !outputs.is_empty() {
                assert_eq!(
                    outputs.len(),
                    if direction == Some(Direction::Both) {
                        4
                    } else {
                        2
                    }
                );
                assert_eq!(after.len() - outputs.len(), usize::from(!delete.unwrap()));
                for output in outputs {
                    if direction != Some(Direction::V) {
                        assert_eq!(output["domain"][0], json!([0., 1.]));
                    }
                    if direction != Some(Direction::U) {
                        assert_eq!(output["domain"][1], json!([0., 1.]));
                    }
                    if direction == Some(Direction::V) {
                        assert_eq!(output["domain"][0], json!([-2., 4.]));
                    }
                    if direction == Some(Direction::U) {
                        assert_eq!(output["domain"][1], json!([10., 18.]));
                    }
                }
            } else {
                assert_eq!(after.len(), 1);
            }
        }
    }
}

#[test]
fn single_span_prompt_preflight_rejects_impossible_stages_and_unseeded_cancellation() {
    let value: Value = serde_json::from_str(CASES).unwrap();
    for changes in [
        json!({"cancel_at_selection":true}),
        json!({"cancel":true,"selected":[]}),
        json!({"initial_selection":[0]}),
        json!({"cancel":true,"postselect":false,"cancel_at_selection":true}),
        json!({"cancel":true,"sources":[{"type":"point","point":[0,0,0]}]}),
    ] {
        let mut op = value["operations"][0].clone();
        op.as_object_mut()
            .unwrap()
            .extend(changes.as_object().unwrap().clone());
        let Operation::SingleSpanConversion { fixture, .. } = serde_json::from_value(op).unwrap()
        else {
            panic!()
        };
        assert!(run_single(&fixture, Tolerance::DEFAULT).is_err());
    }
    let mut request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let Operation::ConversionSession { fixture, .. } = &mut request.operations[0] else {
        panic!()
    };
    fixture.steps[0].conversion.geometry.postselect = true;
    fixture.steps[0].conversion.geometry.cancel = true;
    fixture.steps[0].conversion.geometry.cancel_at_selection = true;
    assert!(run(fixture, Tolerance::DEFAULT).is_err());
}
