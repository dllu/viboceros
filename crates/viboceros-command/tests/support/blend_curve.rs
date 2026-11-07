//! Reconstruct captured inputs without using native result geometry.
use super::{native, reference_inputs};
use serde_json::Value;
use viboceros_command::blend_curve::{Handle, Input};
use viboceros_document::Document;
pub const FAMILIES: [&str; 4] = [
    "blend_curve_pilot",
    "blend_curve_discovery",
    "blend_curve_holdouts",
    "blend_curve_adjustments",
];
pub fn diagnostic(name: &str) -> bool {
    name.starts_with("g3_")
        || name.starts_with("g4_")
        || matches!(
            name,
            "g2_handle_start" | "g1_handle_start" | "flip_by_number"
        )
}
pub fn accepted(value: &Value) -> bool {
    value["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "BlendCrv" && e["result"] == "Nothing")
}
pub fn source_pick(
    value: &Value,
    index: usize,
    action: &Value,
    input: &Input,
) -> viboceros_geometry::Point3 {
    if !input.preferences().pick_point {
        return native::point(&action["pick"]);
    }
    let click = value["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    native::point(&click["sight_point"])
}
pub fn handle(input: &Input, point: viboceros_geometry::Point3) -> Handle {
    input
        .control_points()
        .into_iter()
        .min_by(|a, b| {
            a.1.distance_to(point)
                .unwrap()
                .total_cmp(&b.1.distance_to(point).unwrap())
        })
        .unwrap()
        .0
}
pub fn replay(input: &mut Input, doc: &Document, value: &Value) {
    let mut flip = false;
    for (index, a) in value["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if let Some(i) = a["curve"].as_u64() {
            if flip {
                input.option(&format!("Flip{}=Yes", i + 1)).unwrap();
                flip = false;
            } else {
                input
                    .pick_curve(
                        doc,
                        reference_inputs::guide(doc, i as usize),
                        None,
                        source_pick(value, index, a, input),
                    )
                    .unwrap();
            }
        } else if let Some(p) = a.get("click") {
            input
                .select_handle(handle(input, native::point(p)))
                .unwrap();
        } else if a.is_array() {
            input.adjust_point(native::point(a)).unwrap();
            input.finish_adjustment();
        } else if a == "Flip" {
            flip = true;
        } else if a == "Enter" {
            if flip {
                input.option("Flip1=Yes").unwrap();
                input.option("Flip2=Yes").unwrap();
                flip = false;
            }
        } else if a == "Cancel" {
            break;
        } else {
            assert!(
                input
                    .option_with_units(a.as_str().unwrap(), doc.units())
                    .unwrap()
            );
        }
    }
}
pub fn samples(doc: &Document, value: &Value) -> (f64, usize) {
    let output = value["after"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["attributes"]["name"] == "" || o["native_type"] == "PolyCurve")
        .unwrap();
    let object = doc
        .objects()
        .find(|o| {
            o.attributes().name().is_none()
                || matches!(o.geometry(), viboceros_document::Geometry::PolyCurve(_))
        })
        .unwrap();
    let c = object.geometry().curve_ref().unwrap();
    let target = output["samples"].as_array().unwrap();
    let mut error = 0_f64;
    let [a, b]: [f64; 2] = serde_json::from_value(output["curve"]["domain"].clone()).unwrap();
    for s in target {
        let fraction = (s["parameter"].as_f64().unwrap() - a) / (b - a);
        let domain = c.domain();
        let t = *domain.start() + (domain.end() - domain.start()) * fraction;
        let jet = c.evaluate_with_tangent(t).unwrap();
        error = error.max(jet.point().distance_to(native::point(&s["point"])).unwrap());
        let tangent: [f64; 3] = serde_json::from_value(s["tangent"].clone()).unwrap();
        error = error.max(
            jet.tangent()
                .as_vector()
                .to_array()
                .into_iter()
                .zip(tangent)
                .map(|(x, y)| (x - y).abs())
                .fold(0., f64::max),
        );
    }
    (error, target.len())
}
