use super::*;

#[test]
fn python_fixture_dispatch_preserves_native_output_counts_and_empty_history() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/apply_uv_curves_local.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/apply_uv_curves_command.json"
    ))
    .unwrap();
    for (op, native) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        let op: Operation = serde_json::from_value(op.clone()).unwrap();
        assert_eq!(op.id(), native["id"].as_str().unwrap());
        let Operation::ApplyUvCurves { fixture, .. } = op else {
            panic!()
        };
        let (value, _) = run(&fixture, Tolerance::default(), 1).unwrap();
        let outputs = |rows: &Value| {
            rows.as_array()
                .unwrap()
                .iter()
                .filter(|r| r["source"].is_null())
                .count()
        };
        let count = outputs(&native["value"]["after"]);
        assert_eq!(outputs(&value["after"]), count);
        assert_eq!(outputs(&value["undo"]), 0);
        assert_eq!(outputs(&value["redo"]), count);
        assert_eq!(value["undoable"], count != 0);
        assert_eq!(value["groups_after"], value["groups_after_undo"]);
        assert!(
            value["undo"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["selected"] == false)
        );
    }
}

#[test]
fn python_subcurve_dispatch_runs_all_native_input_recipes_and_history() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/uv_subcurve_input_local.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/uv_subcurve_input_command.json"
    ))
    .unwrap();
    assert_eq!(fixture["operations"].as_array().unwrap().len(), 14);
    for (op, n) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        let op: Operation = serde_json::from_value(op.clone()).unwrap();
        let (v, _) = match op {
            Operation::ApplyUvCurves { fixture, .. } => {
                run(&fixture, Tolerance::DEFAULT, 1).unwrap()
            }
            Operation::CreateUvCurves { fixture, .. } => {
                create(&fixture, Tolerance::DEFAULT, 1).unwrap()
            }
            _ => panic!("unexpected UV operation"),
        };
        let count = n["value"]["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["source"].is_null())
            .count();
        assert_eq!(
            v["after"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["source"].is_null())
                .count(),
            count
        );
        assert_eq!(
            v["groups_after"].as_u64().unwrap() as usize,
            n["value"]["groups_after"].as_array().unwrap().len()
        );
        assert_eq!(v["groups_after"], v["groups_after_undo"]);
        assert_eq!(
            v["redo"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["selected"] == true)
                .count(),
            count
        );
        assert!(
            v["undo"]
                .as_array()
                .unwrap()
                .iter()
                .all(|o| o["selected"] == false)
        );
        let originals = v["before"].as_array().unwrap();
        for after in v["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| !o["source"].is_null())
        {
            let mut before = originals[after["source"].as_u64().unwrap() as usize].clone();
            before["selected"] = after["selected"].clone();
            assert_eq!(after, &before);
        }
    }
}

#[test]
fn python_subcurve_fixture_rejects_bad_ranges_and_group_indices() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/uv_subcurve_input_local.json"
    ))
    .unwrap();
    for (key, value) in [
        ("groups", json!([[999]])),
        ("groups", json!([[]])),
        (
            "inputs",
            json!([{"kind":"curve","name":"partial","definition":fixture["operations"][0]["inputs"][0]["definition"],"subcurves":[[0.,0.]]}]),
        ),
    ] {
        let mut op = fixture["operations"][0].clone();
        op[key] = value;
        let Operation::ApplyUvCurves { fixture, .. } =
            serde_json::from_value::<Operation>(op).unwrap()
        else {
            panic!()
        };
        assert!(run(&fixture, Tolerance::DEFAULT, 1).is_err());
    }
}
