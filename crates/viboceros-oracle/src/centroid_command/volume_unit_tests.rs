use super::*;

#[test]
fn volume_unit_commands_match_source_only_exact_dimensional_witnesses() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/volume_display_units.json"
    ))
    .unwrap();
    let references: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/volume_display_units_reference.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 30);
    for (row, op) in actual.results.iter().zip(&request.operations) {
        let Operation::VolumeCommand { fixture, .. } = op else {
            panic!()
        };
        let declined = fixture.open_confirmation.as_deref() == Some("no");
        assert_eq!(row.value["succeeded"], !declined, "{}", row.id);
        assert_eq!(row.value["points"], json!([]), "{}", row.id);
        assert_eq!(
            row.value["unit_setup"],
            json!(vec![false; fixture.unit_setup.as_ref().unwrap().len()])
        );
        if declined {
            assert_eq!(row.value["volume"], Value::Null);
        } else {
            assert_eq!(
                row.value["volume"], references[&row.id]["rounded_volume"],
                "{}",
                row.id
            );
        }
    }
}

#[test]
fn display_options_are_scalar_only_and_preselected_macros_cannot_change_them() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/volume_display_units.json"
    ))
    .unwrap();
    for change in [
        json!({"op":"volume_centroid_command"}),
        json!({"preselect":true}),
        json!({"model_units":255}),
        json!({"unit_setup":[]}),
        json!({"display_units":"Meter Delete"}),
        json!({"unit_setup":["Meter Continue=Yes"]}),
    ] {
        let mut bad = request.clone();
        bad["operations"][0]
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(run_request(&serde_json::from_value(bad).unwrap()).is_err());
    }
}
