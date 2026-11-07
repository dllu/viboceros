//! Ordered recipes share a command registry; native curves remain expected data.
use super::{inputs, native, reference_inputs};
use serde_json::Value;
use viboceros_command::{
    CommandRegistry,
    arc_blend::{Handle, Input, Preferences},
};
use viboceros_document::Document;
pub const FAMILIES: [&str; 3] = [
    "arc_blend_preferences",
    "arc_blend_preference_holdouts",
    "arc_blend_preference_cancellation",
];
pub fn expected(name: &str) -> Preferences {
    let (radius_difference, trim, join) = match name {
        "prefs_initial" => (0., false, false),
        "prefs_radius_commit"
        | "prefs_manual_reset"
        | "prefs_radius_default"
        | "prefs_nested_cancel"
        | "prefs_zero_recall"
        | "prefs_numeric_cancel"
        | "prefs_numeric_default"
        | "prefs_numeric_seed"
        | "prefs_numeric_twice"
        | "prefs_zero_after_numeric" => (5., false, false),
        "prefs_join_commit"
        | "prefs_join_default"
        | "prefs_join_restored"
        | "prefs_pending_cancel"
        | "prefs_join_after_pending" => (-5., true, true),
        "prefs_trim_cancel" | "prefs_trim_after_cancel" => (-5., false, true),
        "prefs_join_cancel" | "prefs_join_after_cancel" => (-5., true, false),
        "prefs_radius_cancel"
        | "prefs_radius_after_cancel"
        | "prefs_alternate_commit"
        | "prefs_alternate_reset"
        | "prefs_negative_cancel"
        | "prefs_zero_after_cancel" => (-5., false, false),
        _ => panic!("unclassified native settings transition {name}"),
    };
    Preferences {
        radius_difference,
        trim,
        join,
    }
}
pub fn replay(input: &mut Input, doc: &Document, value: &Value, registry: &CommandRegistry) {
    let mut pending = None;
    for a in value["prescribed_case"]["actions"].as_array().unwrap() {
        if let Some(i) = a["curve"].as_u64() {
            input
                .pick_curve(
                    doc,
                    reference_inputs::guide(doc, i as usize),
                    None,
                    native::point(&a["pick"]),
                )
                .unwrap();
        } else if let Some(p) = a.get("click") {
            assert!(input.active_handle().is_none());
            input
                .select_handle(inputs::handle(input, native::point(p)))
                .unwrap();
        } else if a == "RadiusDifference" {
            pending = Some(input.active_handle() == Some(Handle::Ratio));
        } else if a == "Cancel" {
            if let Some(ratio) = pending.take() {
                if ratio {
                    input
                        .remember_radius_difference(input.last_radius_difference())
                        .unwrap();
                } else {
                    input
                        .option(&format!(
                            "RadiusDifference={}",
                            input.last_radius_difference()
                        ))
                        .unwrap();
                }
            }
        } else if a.is_number() || a == "Enter" && pending.is_some() {
            let v = if a == "Enter" {
                input.last_radius_difference()
            } else {
                a.as_f64().unwrap()
            };
            match pending.take() {
                Some(true) => input.remember_radius_difference(v).unwrap(),
                Some(false) => {
                    input.option(&format!("RadiusDifference={v}")).unwrap();
                }
                None => {
                    input.option(&format!("Ratio={v}")).unwrap();
                    input.finish_adjustment();
                }
            }
        } else if a.is_array() {
            input.adjust_point(native::point(a)).unwrap();
            input.finish_adjustment();
        } else if a == "Enter" {
            input.finish_adjustment();
        } else {
            assert!(input.option(a.as_str().unwrap()).unwrap());
        }
        assert!(registry.remember_arc_blend_preferences(input.preferences()));
    }
    assert!(pending.is_none());
}
pub fn native_stations(value: &Value) -> usize {
    if value["success"] == true
        && value["after"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o.get("arc_segments").is_some())
    {
        99
    } else {
        0
    }
}

pub fn diagnostic(name: &str) -> bool {
    name == "prefs_numeric_cancel"
}
