use super::{inputs, native, reference_inputs};
use serde_json::Value;
use viboceros_command::arc_blend::{Handle, Input};
use viboceros_document::Document;
pub const FAMILIES: [&str; 2] = [
    "arc_blend_radius_getters",
    "arc_blend_radius_getter_holdouts",
];
pub fn research(id: &str) -> bool {
    id.starts_with("circle_repeat_")
}
pub fn replay(input: &mut Input, doc: &Document, value: &Value) {
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
            assert!(pending.is_none());
            pending = Some(input.active_handle() == Some(Handle::Ratio));
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
        } else if a == "Enter" {
            input.finish_adjustment();
        } else {
            assert!(input.option(a.as_str().unwrap()).unwrap());
        }
    }
    assert!(pending.is_none());
}
