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
pub fn matches_display(actual: Preferences, displayed: Preferences) -> bool {
    actual.join == displayed.join
        && actual.trim == displayed.trim
        && actual.styles == displayed.styles
        && actual
            .distances
            .into_iter()
            .zip(displayed.distances)
            .all(|(a, b)| (a - b).abs() <= 5e-6 * a.abs().max(b.abs()).max(1.))
}
pub fn final_options(v: &Value) -> Preferences {
    let mut result = options(
        v["input_trace"][0]["prompts"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find_map(|p| p.as_str().filter(|s| s.contains("Distances=")))
            .unwrap(),
    );
    // Native display text rounds distances; accepted numeric input retains its
    // full precision. Getter prompts identify which input was accepted.
    for (action, prompt) in v["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .zip(v["input_trace"][0]["prompts"].as_array().unwrap())
    {
        let prompt = prompt.as_str().unwrap();
        let index = if prompt.starts_with("First chamfer distance") {
            0
        } else if prompt.starts_with("Second chamfer distance") {
            1
        } else {
            continue;
        };
        let text = action
            .as_f64()
            .map(|x| x.to_string())
            .or_else(|| action["number"].as_str().map(str::to_owned));
        if let Some(text) = text {
            if let Ok(distance) = viboceros_command::chamfer::parse_distance(
                &text,
                &viboceros_geometry::LengthUnitSystem::Millimeters,
            ) {
                result.distances[index] = distance;
            }
        } else if index == 1 && action.as_str() == Some("Enter") {
            result.distances[1] = result.distances[0];
        }
    }
    result
}
pub fn unsupported(id: &str) -> bool {
    [
        "closed_two_circles_joined_zero",
        "closed_consumed_line",
        "extension_tangent_arc_cross_source",
        "extension_spatial",
        "extension_xy_line_other_smooth",
        "extension_aligned_plane_line",
        "extension_translated_xy_line",
        "extension_tilted_x_line",
    ]
    .contains(&id)
}
