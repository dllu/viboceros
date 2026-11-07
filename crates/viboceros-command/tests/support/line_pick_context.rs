//! Input calibration from prescribed curves and public pick-frustum transforms.
pub use super::tangent_inputs::{click_point, guide, setup, token};
use super::{native, reference_inputs as curves};
use serde_json::Value;
use viboceros_command::line::curve_input::{Input, Kind};
use viboceros_document::Document;
use viboceros_geometry::Point3;
pub const FAMILIES: [&str; 2] = ["line_pick_context", "line_pick_context_holdouts"];
pub fn sources() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/line_pick_context_sources.json"
    ))
    .unwrap()
}
pub fn excluded(_id: &str) -> bool {
    false
}
pub fn projected_point(v: &Value, index: usize) -> Point3 {
    let click = v["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    let source = &v["prescribed_case"]["curves"][click["curve"].as_u64().unwrap() as usize];
    let geometry = curves::geometry(source);
    let curve = geometry.curve_ref().unwrap().to_nurbs().unwrap();
    let matrix: [[f64; 4]; 4] =
        serde_json::from_value(click["pick_context"]["transform"].clone()).unwrap();
    let t = curve
        .closest_parameter_in_projection([0., 0.], |p| {
            let [x, y, z] = p.to_array();
            let h = [x, y, z, 1.];
            let q: [f64; 4] =
                std::array::from_fn(|i| matrix[i].iter().zip(h).map(|(a, b)| a * b).sum());
            (q[3] > 0.).then(|| [q[0] / q[3], q[1] / q[3]])
        })
        .unwrap();
    curve.evaluate(t).unwrap()
}
pub fn pick(v: &Value, index: usize, _action: &Value) -> Point3 {
    projected_point(v, index)
}
pub fn command(doc: &Document, v: &Value, source: &Value) -> String {
    let mut tokens = vec!["Line".into()];
    if source["planar"] == true {
        tokens.push("Planar=Yes".into());
    }
    let mut input = Input::with_kind(Kind::Tangent, native::frame(v), doc.tolerance());
    for (i, a) in source["actions"].as_array().unwrap().iter().enumerate() {
        if a.get("curve").is_some() {
            let id = guide(doc, a["curve"].as_u64().unwrap() as usize);
            let p = pick(v, i, a);
            tokens.extend([
                format!("Curve={id}"),
                token(&serde_json::json!(p.to_array())),
            ]);
            input.pick_curve(doc, id, None, p).unwrap();
        } else if a.get("click").is_some() {
            let p = click_point(v, i, &input);
            tokens.push(token(&serde_json::json!(p.to_array())));
            input.point(p).unwrap();
        } else {
            tokens.push(token(a));
            if let Some(s) = a.as_str() {
                if s != "Tangent" {
                    assert!(input.option(s).unwrap());
                }
            } else {
                input.number(a.as_f64().unwrap()).unwrap();
            }
        }
    }
    tokens.join(" ")
}
