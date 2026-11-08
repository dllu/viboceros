//! Keep historical geometry witnesses while correcting inferred Undo expectations.
use super::{final_options, native};
use serde_json::{Value, json};

pub fn history_recorded(value: &Value) -> bool {
    value
        .get("verified_history_recorded")
        .unwrap_or(&value["history_changed"])
        .as_bool()
        .unwrap()
}

pub fn read(family: &str) -> Value {
    let mut data = native::read(family);
    let (legacy_case, modern_family, modern_case) = match family {
        "fillet_getter_holdouts" => (
            "zero_joined",
            "fillet_open_zero_pilot",
            "open_zero_meeting_parts",
        ),
        "fillet_getter_final" => (
            "zero_parts",
            "fillet_open_zero_pilot",
            "open_zero_meeting_parts",
        ),
        "chamfer_getter_discovery" => (
            "chamfer_zero_parts",
            "chamfer_open_pilot",
            "open_chamfer_zero_parts",
        ),
        _ => return data,
    };
    let modern = native::read(modern_family);
    let witness = &modern["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == modern_case)
        .unwrap()["value"];
    let value = &mut data["results"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["id"] == legacy_case)
        .unwrap()["value"];
    assert_eq!(
        value["prescribed_case"]["curves"],
        witness["prescribed_case"]["curves"]
    );
    assert_eq!(value["history_changed"], false);
    if family.starts_with("chamfer_") {
        let prompt = value["input_trace"][0]["prompts"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .as_str()
            .unwrap();
        assert!(
            prompt.contains("Distances=0,0")
                && prompt.contains("Join=No")
                && prompt.contains("Trim=Yes")
        );
    } else {
        let options = final_options(value, Default::default());
        assert!(options.radius == 0. && options.trim && !options.join);
    }
    assert_eq!(witness["undo_observation"]["undo_succeeded"], true);
    assert_eq!(witness["undo_observation"]["geometry_changed"], false);
    value["verified_history_recorded"] = witness["history_changed"].clone();
    value["history_correction"] = json!({"family":modern_family,"case":modern_case});
    data
}
