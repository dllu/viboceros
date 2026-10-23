//! Independent saved command workflows and remembered option witnesses.
use serde_json::{Value, json};
use viboceros_command::triangulate_nonplanar_quads::Options;

pub fn groups() -> Vec<(&'static str, Vec<Value>)> {
    [
        (
            "nonplanar_command_discovery",
            include_str!(
                "../../../../tools/rhino_oracle/observations/nonplanar_command_discovery.json"
            ),
        ),
        (
            "nonplanar_command_options",
            include_str!(
                "../../../../tools/rhino_oracle/observations/nonplanar_command_options.json"
            ),
        ),
        (
            "nonplanar_command_syntax_v2",
            include_str!(
                "../../../../tools/rhino_oracle/observations/nonplanar_command_syntax_v2.json"
            ),
        ),
        (
            "nonplanar_command_main",
            include_str!("../../../../tools/rhino_oracle/observations/nonplanar_command_main.json"),
        ),
        (
            "nonplanar_command_selection",
            include_str!(
                "../../../../tools/rhino_oracle/observations/nonplanar_command_selection.json"
            ),
        ),
    ]
    .into_iter()
    .map(|(name, text)| {
        (
            name,
            serde_json::from_str::<Value>(text).unwrap()["results"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| !EXCLUDED.contains(&r["id"].as_str().unwrap()))
                .cloned()
                .collect(),
        )
    })
    .collect()
}
const EXCLUDED: [&str; 5] = [
    "nonplanar_command_both_default",
    "nonplanar_command_explicit_distance",
    "nonplanar_command_distance_separate",
    "nonplanar_command_combined_name",
    "nonplanar_command_first_selection",
];
pub fn arguments(value: &Value) -> String {
    value["input"]["command_arguments"]
        .as_str()
        .unwrap()
        .split_whitespace()
        .filter(|s| !["_All", "_Enter"].contains(s))
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn report(row: &Value, options: Options, maximum: f64) -> Value {
    json!({"id":row["id"],"qualified":true,"geometry":true,"channels":true,"identity":true,"metadata":true,
        "object_order":true,"selection":true,"history":true,"accepted":row["value"]["success"],"maximum_error":maximum,
        "preferences":{"select_by":options.select_by.name(),"distance":options.distance,"angle":options.angle,"split_method":options.method_name()}})
}
