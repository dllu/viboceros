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
pub const HIGH_FAMILIES: [&str; 4] = [
    "blend_curve_discovery",
    "blend_curve_holdouts",
    "blend_curve_high_discovery",
    "blend_curve_high_holdouts",
];
pub fn high(name: &str) -> bool {
    name.starts_with("g3_") || name.starts_with("g4_") || name.starts_with("high_")
}
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
#[path = "blend_curve/picks.rs"]
mod picks;
pub use picks::{source_edge, source_pick};
// Shared UI replays still use world targets; command replays use input projection.
#[allow(dead_code)]
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
    let mut getter = Getter::default();
    for (index, a) in value["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if !getter.action(input, doc, value, index, a) {
            break;
        }
    }
}
#[derive(Default)]
pub struct Getter {
    flip: bool,
    point_prompt: bool,
}
impl Getter {
    pub fn action(
        &mut self,
        input: &mut Input,
        doc: &Document,
        value: &Value,
        index: usize,
        a: &Value,
    ) -> bool {
        if let Some(i) = a["curve"].as_u64() {
            if self.flip {
                let point = source_pick(value, index, a, input);
                input
                    .flip_source_at(reference_inputs::guide(doc, i as usize), point)
                    .unwrap();
                self.flip = false;
            } else {
                input
                    .pick_curve(
                        doc,
                        reference_inputs::guide(doc, i as usize),
                        source_edge(value, index, a),
                        source_pick(value, index, a, input),
                    )
                    .unwrap();
            }
        } else if a.get("click").is_some() {
            if input.active_handle().is_some() {
                if input.angle_constraint().is_some() {
                    input
                        .adjust_point(angle_point(input, value, index))
                        .unwrap();
                } else {
                    input
                        .adjust_shape_length(shape_parameter(input, value, index))
                        .unwrap();
                }
                input.finish_adjustment();
            } else {
                input
                    .select_handle_with_modifiers(
                        handle_at_click(input, value, index),
                        a["shift"].as_bool().unwrap_or(false),
                        a["alt"].as_bool().unwrap_or(false),
                    )
                    .unwrap();
            }
        } else if a.is_array() {
            if self.point_prompt {
                input.pick_point(native::point(a)).unwrap();
                self.point_prompt = false;
            } else {
                input.adjust_point(native::point(a)).unwrap();
                input.finish_adjustment();
            }
        } else if let Some(number) = a["number"].as_str() {
            if input.angle_constraint().is_some() {
                input.adjust_shape_angle(number.parse().unwrap()).unwrap();
            } else if input.secondary_control_adjustment() {
                input.set_shape_distance(number.parse().unwrap()).unwrap();
                return true;
            } else {
                input.adjust_shape_length(number.parse().unwrap()).unwrap();
            }
            input.finish_adjustment();
        } else if a.get("wait_prompt").is_some() {
            // A capture-side prompt fence supplies no geometry or edit.
        } else if a == "Point" {
            self.point_prompt = true;
        } else if a == "Flip" {
            self.flip = true;
        } else if a == "Enter" {
            if self.flip {
                input.flip_both().unwrap();
                self.flip = false;
            }
        } else if a == "Cancel" {
            return false;
        } else {
            let word = a.as_str().unwrap();
            match input.option_with_units(word, doc.units()) {
                Ok(true) => {}
                Err(_)
                    if value["history"]
                        .as_str()
                        .unwrap()
                        .contains(&format!("Unknown command: _{word}")) => {}
                result => panic!("unexpected BlendCrv option result for {word}: {result:?}"),
            }
        }
        true
    }
}
/// The actual projected click, rather than its pre-rounding authored world aim,
/// determines the nearest visible control. Only public input projection enters.
pub fn control_index_at_click(input: &Input, value: &Value, action: usize) -> usize {
    let click = value["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(action as u64))
        .unwrap();
    let matrix: [[f64; 4]; 4] = serde_json::from_value(click["pick_transform"].clone()).unwrap();
    input
        .preview()
        .unwrap()
        .control_points()
        .iter()
        .enumerate()
        .min_by(|a, b| {
            let distance = |point: viboceros_geometry::Point3| {
                let [x, y, z] = point.to_array();
                let p = [x, y, z, 1.];
                let h: [f64; 4] =
                    std::array::from_fn(|i| (0..4).map(|j| matrix[i][j] * p[j]).sum());
                (h[0] / h[3]).hypot(h[1] / h[3])
            };
            distance(a.1.point()).total_cmp(&distance(b.1.point()))
        })
        .unwrap()
        .0
}
pub fn handle_at_click(input: &Input, value: &Value, action: usize) -> Handle {
    let index = control_index_at_click(input, value, action);
    let alt = value["prescribed_case"]["actions"][action]["alt"]
        .as_bool()
        .unwrap_or(false);
    input
        .handle_for_control_point(index, alt)
        .expect("control belongs to an endpoint constraint")
}

/// Intersect an input viewing ray with the independently captured tangent plane.
pub fn angle_point(input: &Input, value: &Value, index: usize) -> viboceros_geometry::Point3 {
    let plane = input.angle_constraint().unwrap();
    let click = value["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    let from = native::point(&click["ray"][0]);
    let to = native::point(&click["ray"][1]);
    let direction = from.vector_to(to).unwrap();
    let t = -plane.signed_distance_to(from).unwrap()
        / plane.normal().as_vector().dot(direction).unwrap();
    from.translated(direction.scaled(t).unwrap()).unwrap()
}
/// Project the public Native click ray onto the active shape constraint.
/// Its measured output curve is not used to construct the edit.
pub fn shape_parameter(input: &Input, value: &Value, index: usize) -> f64 {
    let c = input
        .shape_constraint()
        .expect("physical target requires a shape constraint");
    let click = value["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    let from = native::point(&click["ray"][0]);
    let to = native::point(&click["ray"][1]);
    let point = c
        .project_view_line(
            c.anchor.vector_to(from).unwrap(),
            from.vector_to(to).unwrap(),
        )
        .unwrap();
    c.anchor
        .vector_to(point)
        .unwrap()
        .dot(c.direction.unwrap().as_vector())
        .unwrap()
}

pub fn samples(doc: &Document, value: &Value) -> (f64, usize) {
    let output = value["after"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["attributes"]["name"] == "")
        .or_else(|| {
            value["after"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["native_type"] == "PolyCurve")
        })
        .unwrap();
    let object = doc
        .objects()
        .find(|o| o.attributes().name().is_none())
        .or_else(|| {
            doc.objects()
                .find(|o| matches!(o.geometry(), viboceros_document::Geometry::PolyCurve(_)))
        })
        .unwrap();
    let c = object.geometry().curve_ref().unwrap();
    let target = output["samples"].as_array().unwrap();
    let mut error = 0_f64;
    let [a, b]: [f64; 2] = serde_json::from_value(output["curve"]["domain"].clone()).unwrap();
    for s in target {
        let fraction = (s["parameter"].as_f64().unwrap() - a) / (b - a);
        let t = c.parameter_at(fraction).unwrap();
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

/// Geometry on matching parameter charts; separately retain the native
/// approximate length used as its domain (not a model-space distance).
pub fn check_high(doc: &Document, target: &Value, archive: bool) -> (f64, f64) {
    let mut target = target.clone();
    let mut domain_error = 0_f64;
    for row in target.as_array_mut().unwrap() {
        if row.get("curve").is_none() {
            continue;
        }
        let object = doc
            .objects()
            .find(|o| {
                o.attributes().name().unwrap_or("") == row["attributes"]["name"].as_str().unwrap()
            })
            .unwrap();
        let actual = object.geometry().curve_ref().unwrap().to_nurbs().unwrap();
        let [a, b]: [f64; 2] = serde_json::from_value(row["curve"]["domain"].clone()).unwrap();
        let domain = actual.domain();
        domain_error = domain_error
            .max((domain.start() - a).abs())
            .max((domain.end() - b).abs());
        for knot in row["curve"]["knots"].as_array_mut().unwrap() {
            let fraction = (knot.as_f64().unwrap() - a) / (b - a);
            *knot = serde_json::json!(domain.start() + (domain.end() - domain.start()) * fraction);
        }
        row["curve"]["domain"] = serde_json::json!([domain.start(), domain.end()]);
    }
    (super::inputs::check(doc, &target, archive), domain_error)
}

/// Public Native derivatives, including the analytic source charts, evaluated
/// on equal fractions and expressed on the same normalized parameter interval.
pub fn derivatives(doc: &Document, target: &Value) -> (f64, f64, usize) {
    use viboceros_geometry::ParameterSide;
    let mut absolute = 0_f64;
    let mut relative = 0_f64;
    let mut stations = 0;
    for row in target.as_array().unwrap() {
        let Some(samples) = row.get("derivative_samples") else {
            continue;
        };
        let object = doc
            .objects()
            .find(|o| {
                o.attributes().name().unwrap_or("") == row["attributes"]["name"].as_str().unwrap()
            })
            .unwrap();
        let c = object.geometry().curve_ref().unwrap();
        let [a, b]: [f64; 2] = serde_json::from_value(row["curve"]["domain"].clone()).unwrap();
        let domain = c.domain();
        for sample in samples.as_array().unwrap() {
            let fraction = (sample["parameter"].as_f64().unwrap() - a) / (b - a);
            let t = c.parameter_at(fraction).unwrap();
            let (point, jet) = c
                .evaluate_with_fourth_derivatives_on_side(
                    t,
                    if fraction == 1. {
                        ParameterSide::Left
                    } else {
                        ParameterSide::Right
                    },
                )
                .unwrap();
            let values = std::iter::once(point.to_array()).chain(jet.map(|d| d.to_array()));
            for (order, (actual, expected)) in values
                .zip(sample["derivatives"].as_array().unwrap())
                .enumerate()
            {
                let expected: [f64; 3] = serde_json::from_value(expected.clone()).unwrap();
                for k in 0..3 {
                    let actual = actual[k] * (domain.end() - domain.start()).powi(order as i32);
                    let expected = expected[k] * (b - a).powi(order as i32);
                    let error = (actual - expected).abs();
                    absolute = absolute.max(error);
                    relative = relative.max(error / actual.abs().max(expected.abs()).max(1.));
                }
            }
            stations += 1;
        }
    }
    (absolute, relative, stations)
}
