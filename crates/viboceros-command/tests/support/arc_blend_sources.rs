//! Reconstruct source/input recipes without consuming native output geometry.
use super::{inputs, native, reference_inputs};
use serde_json::Value;
use viboceros_command::arc_blend::{Handle, Input, parse_number};
use viboceros_document::Document;
use viboceros_geometry::{CurveRef, CurveSegment3, LengthUnitSystem};
pub const FAMILIES: [&str; 3] = [
    "arc_blend_length_discovery",
    "arc_blend_sources",
    "arc_blend_source_holdouts",
];
pub fn diagnostic(id: &str) -> bool {
    id.starts_with("length_") && id != "length_control" || id == "quantity_nested_unit"
}
pub fn setup(value: &Value) -> Document {
    let mut doc = reference_inputs::setup(value, &value["prescribed_case"]);
    let units = match value["model_units"].as_str() {
        Some("Meters") => LengthUnitSystem::Meters,
        Some("Inches") => LengthUnitSystem::Inches,
        _ => LengthUnitSystem::Millimeters,
    };
    doc.set_units(units, false).unwrap();
    doc.clear_undo_records().unwrap();
    doc
}
pub fn replay(input: &mut Input, doc: &Document, value: &Value) {
    let mut radius = None;
    for (index, action) in value["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
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
            if input.active_handle().is_some() {
                let t = constraint_parameter(value, index, input);
                input.adjust_constraint_parameter(t).unwrap();
                input.finish_adjustment();
            } else {
                input
                    .select_handle(inputs::handle(input, native::point(p)))
                    .unwrap();
            }
        } else if action == "RadiusDifference" {
            radius = Some(input.active_handle() == Some(Handle::Ratio));
        } else if action.is_array() {
            input.adjust_point(native::point(action)).unwrap();
            input.finish_adjustment();
        } else if action.is_number() || action.get("number").is_some() {
            let text = action["number"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| action.to_string());
            match radius.take() {
                Some(true) => input
                    .remember_radius_difference(parse_number(&text, doc.units()).unwrap())
                    .unwrap(),
                kind => {
                    let option = if kind == Some(false) {
                        "RadiusDifference"
                    } else {
                        "Ratio"
                    };
                    input
                        .option_with_units(&format!("{option}={text}"), doc.units())
                        .unwrap();
                    input.finish_adjustment();
                }
            }
        } else if action != "Enter" {
            input
                .option_with_units(action.as_str().unwrap(), doc.units())
                .unwrap();
        }
    }
    assert!(radius.is_none());
}
pub fn samples(doc: &Document, value: &Value) -> (f64, usize) {
    let output = value["after"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["attributes"]["name"] == "")
        .unwrap();
    let Some(arcs) = output.get("arc_segments") else {
        return (0., 0);
    };
    let object = doc
        .objects()
        .find(|o| o.attributes().name().is_none())
        .unwrap();
    let viboceros_document::Geometry::PolyCurve(curve) = object.geometry() else {
        panic!("expected polycurve")
    };
    let mut error = super::radius::samples(CurveRef::PolyCurve(curve), output);
    assert_eq!(curve.segments().len(), arcs.as_array().unwrap().len());
    for (segment, target) in curve.segments().iter().zip(arcs.as_array().unwrap()) {
        let CurveSegment3::Arc(arc) = segment else {
            panic!("expected arc")
        };
        error = error.max((arc.radius() - target["radius"].as_f64().unwrap()).abs());
        error = error.max(super::radius::samples(CurveRef::Arc(arc), target));
    }
    (error, 33 * (arcs.as_array().unwrap().len() + 1))
}

/// Native getter words become explicit scripting options, retaining their text.
pub fn command(input: &Input, name: &str, value: &Value) -> String {
    let args = input.arguments().unwrap();
    if !(name.starts_with("quantity_") || name.starts_with("ratio_") || name == "length_control") {
        return args;
    }
    let mut words = args
        .split_whitespace()
        .filter(|w| !w.starts_with("Ratio=") && !w.starts_with("RadiusDifference="))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let action = value["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a.get("number").is_some() || a.is_number())
        .unwrap();
    let number = action["number"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| action.to_string());
    let key = if name.starts_with("ratio_") {
        "Ratio"
    } else {
        "RadiusDifference"
    };
    words.push(format!("{key}={number}"));
    if name == "quantity_nested" {
        words.push("Ratio=0".into());
    }
    words.join(" ")
}

pub fn constraint_parameter(value: &Value, index: usize, input: &Input) -> f64 {
    if input.active_handle() == Some(Handle::Ratio) {
        return inputs::parameter(value, index, input);
    }
    let click = value["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    let matrix: [[f64; 4]; 4] = serde_json::from_value(click["pick_transform"].clone()).unwrap();
    input
        .adjustment_curve()
        .unwrap()
        .closest_parameter_in_projection([0., 0.], |p| {
            let [x, y, z] = p.to_array();
            let h = [x, y, z, 1.];
            let q: [f64; 4] =
                std::array::from_fn(|i| matrix[i].iter().zip(h).map(|(a, b)| a * b).sum());
            (q[3] > 0.).then(|| [q[0] / q[3], q[1] / q[3]])
        })
        .unwrap()
}

pub fn target(value: &Value) -> Option<f64> {
    let line = value["history"]
        .as_str()
        .unwrap()
        .lines()
        .rfind(|s| s.starts_with("Select arc blend point to adjust."))?;
    line.split_once("RadiusDifference=").map(|(_, text)| {
        text.split_whitespace()
            .next()
            .unwrap()
            .trim_end_matches([')', ':'])
            .parse()
            .unwrap()
    })
}
pub fn check_target(actual: Option<f64>, expected: Option<f64>) {
    match (actual, expected) {
        (Some(a), Some(b)) => assert!(
            a.is_finite() && (a - b).abs() <= 1e-12 * b.abs().max(1.),
            "requested radius {a}, native target {b}"
        ),
        _ => assert_eq!(actual, expected),
    }
}
