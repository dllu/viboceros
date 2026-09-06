use super::*;

const SINGLE: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/single_span_conversion.json");
const SESSION: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/conversion_sessions.json");

#[test]
fn permanent_single_span_records_fresh_attributes_unit_split_axes_and_noops() {
    let request: ProbeRequest = serde_json::from_str(SINGLE).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 78);
    for (op, result) in request.operations.iter().zip(response.results) {
        let Operation::SingleSpanConversion { fixture: f, .. } = op else {
            panic!()
        };
        let before = &result.value["before"];
        let after = &result.value["after"];
        assert_eq!(after["groups"].as_array().unwrap().len(), 3);
        let objects = after["objects"].as_array().unwrap();
        let outputs = objects
            .iter()
            .filter(|o| o["original"].is_null())
            .collect::<Vec<_>>();
        if outputs.is_empty() {
            assert_eq!(before, after);
            continue;
        }
        for output in outputs {
            assert_eq!(output["kind"], "surface");
            assert_eq!(output["layer"], "Current");
            assert_eq!(output["groups"], json!([]));
            assert_eq!(output["selected"], false);
            assert!(output["name"].is_null());
            let axes = match toggled(f.direction.unwrap(), f.toggles) {
                Direction::U => vec![0],
                Direction::V => vec![1],
                Direction::Both => vec![0, 1],
            };
            for axis in axes {
                assert_eq!(output["domain"][axis], json!([0., 1.]));
                assert_eq!(
                    output["definition"]["control_count"][axis]
                        .as_u64()
                        .unwrap(),
                    output["definition"]["degree"][axis].as_u64().unwrap() + 1
                );
            }
        }
    }
}

#[test]
fn self_seeded_sessions_retain_independent_options_and_noop_choices() {
    let request: ProbeRequest = serde_json::from_str(SESSION).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 8);
    for (op, result) in request.operations.iter().zip(response.results) {
        let Operation::ConversionSession { fixture: f, .. } = op else {
            panic!()
        };
        let states = result.value["states"].as_array().unwrap();
        assert_eq!(states.len(), f.steps.len());
        let mut choices = BTreeMap::new();
        let mut direction = None;
        for (step, state) in f.steps.iter().zip(states) {
            if let Some(value) = step.conversion.geometry.delete_input {
                choices.insert(step.command, value);
            }
            if step.command == ConversionCommand::ConvertToSingleSpans {
                direction = Some(toggled(
                    step.conversion.direction.or(direction).unwrap(),
                    step.conversion.toggles,
                ));
            }
            let after = state["after"]["objects"].as_array().unwrap();
            let converted = after.iter().any(|o| o["original"].is_null());
            if converted {
                assert_eq!(
                    after.iter().any(|o| !o["original"].is_null()),
                    !choices[&step.command]
                );
            }
            if step.command == ConversionCommand::ConvertToSingleSpans {
                for output in after.iter().filter(|o| o["original"].is_null()) {
                    let axes = match direction.unwrap() {
                        Direction::U => vec![0],
                        Direction::V => vec![1],
                        Direction::Both => vec![0, 1],
                    };
                    for axis in axes {
                        assert_eq!(output["domain"][axis], json!([0., 1.]));
                    }
                }
            }
        }
    }
}

fn toggled(direction: Direction, toggles: u8) -> Direction {
    if toggles.is_multiple_of(2) {
        return direction;
    }
    match direction {
        Direction::U => Direction::V,
        Direction::V => Direction::U,
        Direction::Both => panic!("Both cannot toggle"),
    }
}

#[test]
fn sessions_reject_missing_seeds_and_excessive_steps() {
    let request: ProbeRequest = serde_json::from_str(SESSION).unwrap();
    let Operation::ConversionSession { fixture, .. } = &request.operations[0] else {
        panic!()
    };
    let mut f = fixture.clone();
    f.steps[0].conversion.geometry.delete_input = None;
    assert!(run(&f, Tolerance::DEFAULT).is_err());
    f.steps.clear();
    assert!(run(&f, Tolerance::DEFAULT).is_err());
    f.steps = vec![fixture.steps[0].clone(); 33];
    assert!(run(&f, Tolerance::DEFAULT).is_err());
    f.steps = vec![fixture.steps[0].clone(); 2];
    for step in &mut f.steps {
        step.command = ConversionCommand::ConvertToSingleSpans;
    }
    f.steps[0].conversion.direction = Some(Direction::Both);
    f.steps[1].conversion.direction = None;
    f.steps[1].conversion.toggles = 1;
    assert!(run(&f, Tolerance::DEFAULT).is_err());
    assert!(run_single(&f.steps[1].conversion, Tolerance::DEFAULT).is_err());
}
