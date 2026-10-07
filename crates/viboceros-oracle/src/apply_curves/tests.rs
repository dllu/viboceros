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
