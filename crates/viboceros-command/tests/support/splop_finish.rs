//! Actual Enter/Cancel completion, without a following idle Cancel command.
use serde_json::Value;
pub fn rows() -> Vec<(Value, Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/splop_finish.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/splop_finish.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 48);
    assert_eq!(r["results"].as_array().unwrap().len(), 48);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            assert!(row["error"].is_null());
            (
                op.clone(),
                row["value"]["recipe"].clone(),
                row["value"].clone(),
            )
        })
        .collect()
}
