//! Replay source parameters solely from prescribed getter inputs and public click matrices.
use super::{native, reference_inputs};
use serde_json::Value;
use viboceros_command::{curve_reference::CapturedCurve, subcurve_input};
use viboceros_document::Document;
#[derive(Default)]
struct Pending {
    source: Option<CapturedCurve>,
    start: Option<f64>,
    length: Option<f64>,
    hover: Option<f64>,
    locked: Option<bool>,
}
// BlendCrv also uses the action index for public source-pick receipts.
#[allow(dead_code)]
pub enum Step<'a> {
    Portion(viboceros_command::curve_reference::PortionBinding),
    Input(usize, &'a Value),
}
pub fn steps<'a>(doc: &Document, value: &'a Value) -> Vec<Step<'a>> {
    let mut steps = vec![];
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
        if action == "Cancel" && pending.is_some() {
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
                let confirmation = p.locked.map(|forward| {
                    subcurve_input::locked_numeric_requires_confirmation(
                        c,
                        p.start.unwrap(),
                        length,
                        forward,
                        doc.tolerance(),
                    )
                    .unwrap()
                });
                if confirmation != Some(false) {
                    if confirmation == Some(true) {
                        p.locked = None;
                    }
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
                steps.push(Step::Portion(
                    viboceros_command::curve_reference::PortionBinding {
                        object: source.object_id(),
                        edge: source.source().1,
                        parameters: range,
                        wrap: p.length.is_some(),
                    },
                ));
            }
            pending = None;
            continue;
        }
        steps.push(Step::Input(index, action));
    }
    steps
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
