use serde_json::Value;
use std::collections::BTreeSet;
pub fn normalize(state: &Value) -> Value {
    let components = state["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            // These owned quad surfaces have one trim per edge in the same order.
            // Rhino also marks trims during chain preview; the shared model selects
            // the corresponding model-space edge rather than a duplicate UV marker.
            assert!(matches!(c["kind"].as_str(), Some("BrepTrim" | "BrepEdge")));
            (c["object"].as_u64().unwrap(), c["index"].as_u64().unwrap())
        })
        .collect::<BTreeSet<_>>();
    serde_json::json!({"whole":state["whole"],"objects":state["objects"],"components":components.into_iter().map(|(object,index)|serde_json::json!({"object":object,"kind":"BrepEdge","index":index})).collect::<Vec<_>>()})
}
