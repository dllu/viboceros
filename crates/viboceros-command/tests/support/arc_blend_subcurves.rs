//! Replay frozen getter inputs; expected geometry never seeds construction.
use super::{inputs, native, reference_inputs};
use serde_json::Value;
use viboceros_command::{arc_blend::Input, curve_reference::CapturedCurve, subcurve_input};
use viboceros_document::Document;
pub const FAMILIES: [&str; 4] = [
    "arc_blend_subcurve_discovery",
    "arc_blend_subcurve_holdouts",
    "arc_blend_subcurve_pointers",
    "arc_blend_subcurve_closed",
];
#[derive(Default)]
struct Pending {
    source: Option<CapturedCurve>,
    start: Option<f64>,
    length: Option<f64>,
    hover: Option<f64>,
    locked: Option<bool>,
}
pub fn replay(input: &mut Input, doc: &Document, value: &Value) {
    let mut pending: Option<Pending> = None;
    for (index, action) in value["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if action == "SubCrv" {
            pending = Some(Pending::default());
            continue;
        }
        if action == "Cancel" {
            pending = None;
            continue;
        }
        if let Some(p) = pending.as_mut() {
            if let Some(i) = action["curve"].as_u64() {
                p.source = Some(
                    CapturedCurve::capture(doc, reference_inputs::guide(doc, i as usize), None)
                        .unwrap(),
                );
                continue;
            }
            let source = p.source.as_ref().unwrap();
            let c = source.curve_ref();
            if let Some(point) = action.get("hover") {
                p.hover = Some(
                    c.closest_parameter(native::point(point), doc.tolerance())
                        .unwrap(),
                );
                continue;
            }
            if let Some(word) = action.as_str().and_then(subcurve_input::direction_option) {
                let locked = word.unwrap();
                p.locked = locked.then(|| {
                    subcurve_input::cursor_forward(c, p.start.unwrap(), p.hover.unwrap()).unwrap()
                });
                continue;
            }
            let t = if action.is_array() {
                Some(
                    c.closest_parameter(native::point(action), doc.tolerance())
                        .unwrap(),
                )
            } else if action.get("click").is_some() {
                let rational = pointer_parameter(value, index, source.curve());
                Some(c.parameter_from_nurbs(rational).unwrap())
            } else {
                None
            };
            if p.start.is_none() {
                p.start = t;
                continue;
            }
            if action.is_number() || action.get("number").is_some() {
                let text = action["number"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| action.to_string());
                let length =
                    viboceros_drafting::PointInput::parse_length_with_units(&text, doc.units())
                        .unwrap()
                        .unwrap()
                        .abs();
                if length == 0. {
                    pending = None;
                    continue;
                }
                p.length = Some(length);
                if p.locked.is_none() {
                    continue;
                }
            }
            let range = if let Some(length) = p.length {
                if let Some(forward) = p.locked {
                    subcurve_input::interval_in_direction(
                        c,
                        p.start.unwrap(),
                        length,
                        forward,
                        doc.tolerance(),
                    )
                    .unwrap()
                } else {
                    subcurve_input::interval(
                        c,
                        p.start.unwrap(),
                        t.unwrap(),
                        length,
                        doc.tolerance(),
                    )
                    .unwrap()
                }
            } else {
                subcurve_input::point_parameters(c, p.start.unwrap(), t.unwrap(), p.locked, false)
                    .unwrap()
            };
            if let Some(range) = range {
                input
                    .pick_subcurve(
                        doc,
                        source.object_id(),
                        source.source().1,
                        range,
                        p.length.is_some(),
                    )
                    .unwrap();
            }
            pending = None;
            continue;
        }
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
        } else if action != "Enter" {
            input
                .option_with_units(action.as_str().unwrap(), doc.units())
                .unwrap();
        }
    }
}

/// Reconstruct the native click ray from its public projection, without using
/// any output curve or fitted contact as an input.
pub fn pointer_parameter(
    value: &Value,
    index: usize,
    curve: &viboceros_geometry::NurbsCurve,
) -> f64 {
    let click = value["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    let matrix: [[f64; 4]; 4] = serde_json::from_value(click["pick_transform"].clone()).unwrap();
    curve
        .closest_parameter_in_projection([0., 0.], |p| {
            let h = [p.x(), p.y(), p.z(), 1.];
            let q: [f64; 4] =
                std::array::from_fn(|i| matrix[i].iter().zip(h).map(|(a, b)| a * b).sum());
            (q[3] > 0.).then(|| [q[0] / q[3], q[1] / q[3]])
        })
        .unwrap()
}
