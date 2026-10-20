//! Shared native reference-sphere inputs, captured as executed public macros.
use serde_json::{Value, json};
pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/splop_reference.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/splop_reference.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 21);
    assert_eq!(r["results"].as_array().unwrap().len(), 21);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, r)| {
            assert_eq!(op["id"], r["id"]);
            assert!(r["error"].is_null());
            (op.clone(), r["value"].clone())
        })
        .collect()
}
pub fn mapped(op: &Value) -> Value {
    json!({"id":op["id"],"shape":"Cloud","surface":"Plane","plane":op["plane"],"copy":false,"rigid":false,"flip":false,"grouped":false,"angle":0.,"radius":4.})
}
pub fn tokens(value: &Value) -> Vec<&str> {
    value["executed_macro"]
        .as_str()
        .unwrap()
        .strip_prefix("_Splop ")
        .unwrap()
        .split(" '_-SelID")
        .next()
        .unwrap()
        .split_whitespace()
        .collect()
}
pub fn reference(op: &Value, value: &Value) -> viboceros_command::circle_input::CircleDefinition {
    let mut input = viboceros_command::splop::ReferenceInput::new(
        crate::splop_test_support::frame(op["plane"].as_str().unwrap()),
    );
    let mut definition = None;
    for token in tokens(value) {
        assert!(definition.is_none());
        if input.option(token) {
            continue;
        }
        definition = if let Some(p) = token.strip_prefix('w') {
            let p = p.split(',').map(|x| x.parse().unwrap()).collect::<Vec<_>>();
            input
                .point(viboceros_geometry::Point3::try_from([p[0], p[1], p[2]]).unwrap())
                .unwrap()
        } else {
            input.number(token.parse().unwrap()).unwrap()
        };
    }
    definition.unwrap()
}
