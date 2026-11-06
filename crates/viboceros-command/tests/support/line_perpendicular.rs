//! Prescribed source construction and calibrated physical curve-pick replay.
use super::{native, reference_inputs as curves};
pub use curves::{guide, token};
use serde_json::Value;
use viboceros_document::{Document, ObjectId};
pub const FAMILIES: [&str; 5] = [
    "line_perpendicular",
    "line_perpendicular_holdouts",
    "line_perpendicular_final",
    "line_perpendicular_completion",
    "line_perpendicular_boundaries",
];
pub fn sources() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/line_perpendicular_sources.json"
    ))
    .unwrap()
}
pub fn diagnostic(id: &str) -> bool {
    [
        "from_first_early",
        "oncurve_fixed",
        "oncurve_fixed_length",
        "planar_spatial",
        "planar_spatial_fixed",
        "numeric_enter_cancel",
        "planar_mouse_sliding",
        "planar_mouse_fixed",
    ]
    .contains(&id)
}
pub fn setup(v: &Value, source: &Value) -> Document {
    curves::setup(v, source)
}
/// The public SDK measured the curve under the physical mouse ray before the
/// click. This calibrates an input station, independently of output geometry.
pub fn pick(v: &Value, index: usize, _action: &Value) -> viboceros_geometry::Point3 {
    let c = v["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    native::point(c.get("sight_point").unwrap_or(&c["projected_point"]))
}
pub fn click_point(
    v: &Value,
    index: usize,
    input: &viboceros_command::line::perpendicular::Input,
) -> viboceros_geometry::Point3 {
    let c = v["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    let raw = native::point(&c["projected_point"]);
    if let Some(plane) = input.mouse_plane() {
        let origin = native::point(&c["ray"][0]);
        let direction = origin.vector_to(native::point(&c["ray"][1])).unwrap();
        let n = plane.z_axis().as_vector();
        let denominator = direction.dot(n).unwrap();
        if denominator.abs() > 1e-12 * direction.length().unwrap() {
            let t = origin.vector_to(plane.origin()).unwrap().dot(n).unwrap() / denominator;
            return origin.translated(direction.scaled(t).unwrap()).unwrap();
        }
    }
    raw
}
pub fn command(doc: &Document, v: &Value, source: &Value) -> String {
    let mut tokens = vec!["Line".into()];
    let mut input =
        viboceros_command::line::perpendicular::Input::new(native::frame(v), doc.tolerance());
    input.set_drafting(native::frame(v), source["planar"] == true);
    if source["planar"] == true {
        tokens.push("Planar=Yes".into());
    }
    for (i, a) in source["actions"].as_array().unwrap().iter().enumerate() {
        if a.get("curve").is_some() {
            let id: ObjectId = guide(doc, a["curve"].as_u64().unwrap() as usize);
            tokens.push(format!("Curve={id}"));
            let p = pick(v, i, a);
            tokens.push(token(&serde_json::json!(p.to_array())));
            input.pick_curve(doc, id, None, p).unwrap();
        } else if a.get("click").is_some() {
            let p = click_point(v, i, &input);
            tokens.push(token(&serde_json::json!(p.to_array())));
            input.point(p).unwrap();
        } else {
            tokens.push(token(a));
            if a.is_array() {
                input.point(native::point(a)).unwrap();
            } else if let Some(s) = a.as_str() {
                if s == "Enter" {
                    assert!(input.enter());
                } else if s != "Perpendicular" {
                    assert!(input.option(s).unwrap());
                }
            } else {
                input.number(a.as_f64().unwrap()).unwrap();
            }
        }
    }
    tokens.join(" ")
}
