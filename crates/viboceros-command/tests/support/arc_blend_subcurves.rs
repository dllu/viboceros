//! Replay frozen getter inputs; expected geometry never seeds construction.
use super::{inputs, native, reference_inputs};
use serde_json::Value;
use viboceros_command::arc_blend::Input;
use viboceros_document::Document;
pub const FAMILIES: [&str; 4] = [
    "arc_blend_subcurve_discovery",
    "arc_blend_subcurve_holdouts",
    "arc_blend_subcurve_pointers",
    "arc_blend_subcurve_closed",
];
#[path = "curve_portion_input.rs"]
mod portion;
#[allow(unused_imports)]
pub use portion::pointer_parameter;
pub fn replay(input: &mut Input, doc: &Document, value: &Value) {
    for step in portion::steps(doc, value) {
        let action = match step {
            portion::Step::Portion(p) => {
                input
                    .pick_subcurve(doc, p.object, p.edge, p.parameters, p.wrap)
                    .unwrap();
                continue;
            }
            portion::Step::Input(_, action) => action,
        };
        if let Some(i) = action["curve"].as_u64() {
            input
                .pick_curve(
                    doc,
                    reference_inputs::guide(doc, i as usize),
                    None,
                    native::point(&action["pick"]),
                )
                .unwrap();
        } else if let Some(p) = action.get("click") {
            input
                .select_handle(inputs::handle(input, native::point(p)))
                .unwrap();
        } else if action.is_array() {
            input.adjust_point(native::point(action)).unwrap();
            input.finish_adjustment();
        } else if action != "Enter" && action != "Cancel" {
            input
                .option_with_units(action.as_str().unwrap(), doc.units())
                .unwrap();
        }
    }
}
