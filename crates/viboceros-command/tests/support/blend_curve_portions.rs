//! Bounded and seam-wrapping temporary inputs never use native outputs as seeds.
use super::{blend, native, reference_inputs};
#[path = "curve_portion_input.rs"]
mod portion;
#[allow(unused_imports)]
pub use portion::pointer_parameter;
#[path = "blend_curve_portion_families.rs"]
mod families;
pub use families::FAMILIES;

pub fn replay(
    input: &mut viboceros_command::blend_curve::Input,
    doc: &viboceros_document::Document,
    value: &serde_json::Value,
) {
    let mut getter = blend::Getter::default();
    for step in portion::steps(doc, value) {
        match step {
            portion::Step::Portion(p) => input
                .pick_subcurve(doc, p.object, p.edge, p.parameters, p.wrap)
                .unwrap(),
            portion::Step::Input(index, a) => {
                if !getter.action(input, doc, value, index, a) {
                    break;
                }
            }
        }
    }
}
