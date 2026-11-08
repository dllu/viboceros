//! Keep historical geometry witnesses while correcting two inferred Undo expectations.
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
    let legacy_case = match family {
        "fillet_getter_holdouts" => "zero_joined",
        "fillet_getter_final" => "zero_parts",
        _ => return data,
    };
    let modern = native::read("fillet_open_zero_pilot");
    let witness = &modern["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "open_zero_meeting_parts")
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
    let options = final_options(value, Default::default());
    assert!(options.radius == 0. && options.trim && !options.join);
    assert_eq!(witness["undo_observation"]["undo_succeeded"], true);
    assert_eq!(witness["undo_observation"]["geometry_changed"], false);
    value["verified_history_recorded"] = witness["history_changed"].clone();
    value["history_correction"] =
        json!({"family":"fillet_open_zero_pilot","case":"open_zero_meeting_parts"});
    data
}
