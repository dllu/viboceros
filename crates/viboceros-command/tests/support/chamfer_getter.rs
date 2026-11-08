//! Public getter options supply inputs; output geometry is only an expectation.
use serde_json::Value;
use viboceros_command::chamfer::Preferences;
pub const FAMILIES: [&str; 3] = [
    "chamfer_getter_pilot",
    "chamfer_getter_discovery",
    "chamfer_getter_holdouts",
];
pub fn options(prompt: &str) -> Preferences {
    let words: Vec<_> = prompt
        .split_whitespace()
        .filter(|s| s.contains('='))
        .collect();
    Preferences::default()
        .updated(&words, &viboceros_geometry::LengthUnitSystem::Millimeters)
        .unwrap()
}
pub fn initial(v: &Value) -> Preferences {
    options(v["input_trace"][0]["prompts"][0].as_str().unwrap())
}
pub fn final_options(v: &Value) -> Preferences {
    options(
        v["input_trace"][0]["prompts"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find_map(|p| p.as_str().filter(|s| s.contains("Distances=")))
            .unwrap(),
    )
}
pub fn unsupported(id: &str) -> bool {
    [
        "chamfer_oversized",
        "closed_two_circles_joined_zero",
        "closed_consumed_line",
    ]
    .contains(&id)
}
