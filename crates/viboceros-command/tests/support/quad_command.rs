//! Exact native command geometry, channels, object order and lifecycle fixtures.
use super::native;
use serde_json::{Value, json};
use viboceros_command::quadrangulate_mesh::Options;

pub fn groups() -> Vec<(&'static str, Vec<Value>)> {
    [
        (
            "quad_command_discovery",
            include_str!("../../../../tools/rhino_oracle/observations/quad_command_discovery.json"),
        ),
        (
            "quad_command_main",
            include_str!("../../../../tools/rhino_oracle/observations/quad_command_main.json"),
        ),
    ]
    .into_iter()
    .map(|(family, text)| {
        (
            family,
            serde_json::from_str::<Value>(text).unwrap()["results"]
                .as_array()
                .unwrap()
                .clone(),
        )
    })
    .collect()
}
pub fn arguments(value: &Value) -> String {
    value["input"]["command_arguments"]
        .as_str()
        .unwrap()
        .split_whitespace()
        .filter(|w| !["_All", "_Enter"].contains(w))
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn command(value: &Value) -> String {
    format!("QuadrangulateMesh {}", arguments(value))
}
pub use native::mesh_command::check;
pub fn report(row: &Value, options: Options, maximum: f64) -> Value {
    json!({"id":row["id"],"qualified":true,"geometry":true,"channels":true,"identity":true,
        "metadata":true,"object_order":true,"selection":true,"history":true,"maximum_error":maximum,
        "accepted":row["value"]["success"],"preferences":{"planarity":options.planarity,"rectangularity":options.rectangularity}})
}
