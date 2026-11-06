//! Prescribed source construction and calibrated physical curve-pick replay.
use super::{native, reference_inputs as curves};
pub use curves::{guide, token};
use serde_json::Value;
use viboceros_document::{Document, ObjectId};
pub const FAMILIES: [&str; 4] = [
    "line_tangent",
    "line_tangent_holdouts",
    "line_tangent_final",
    "line_tangent_completion",
];
pub fn sources() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/line_tangent_sources.json"
    ))
    .unwrap()
}
pub fn diagnostic(id: &str) -> bool {
    [
        "line_tangent",
        "circle_spatial_end",
        "circle_inside",
        "arc_tangent",
        "quadratic_tangent",
        "spatial_tangent",
        "tangent_both",
        "numeric_then_point",
        "numeric_only",
        "fixed_numeric",
        "arc_planar_tangent",
        "arc_right",
        "planar_circle",
        "oncurve_both_length",
        "arc_middle_repeat_a",
        "arc_middle_repeat_b",
        "oncurve_negative",
        "fixed_negative",
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
    input: &viboceros_command::line::curve_input::Input,
) -> viboceros_geometry::Point3 {
    let c = v["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    let raw = native::point(&c["projected_point"]);
    if c["ray"].is_array()
        && let Some(line) = input.mouse_line()
    {
        let origin = native::point(&c["ray"][0]);
        let direction = origin.vector_to(native::point(&c["ray"][1])).unwrap();
        return line
            .project_view_line(line.anchor.vector_to(origin).unwrap(), direction)
            .unwrap();
    }
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
    let mut input = viboceros_command::line::curve_input::Input::with_kind(
        viboceros_command::line::curve_input::Kind::Tangent,
        native::frame(v),
        doc.tolerance(),
    );
    input.set_drafting(native::frame(v), source["planar"] == true);
    if source["planar"] == true {
        tokens.push("Planar=Yes".into());
    }
    for (i, a) in source["actions"].as_array().unwrap().iter().enumerate() {
        // In a complete native trace, two successive endpoint point tokens
        // mean the first was rejected while the same getter remained active.
        if a.is_array()
            && source["actions"].get(i + 1).is_some_and(|n| n.is_array())
            && v["input_trace"][0]["prompts"][i]
                .as_str()
                .is_some_and(|p| p.starts_with("End of line"))
        {
            assert!(input.point(native::point(a)).is_err());
            continue;
        }

        if a.get("hover").is_some() {
            continue;
        }
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
                } else if s != "Tangent" {
                    assert!(input.option(s).unwrap());
                }
            } else {
                input.number(a.as_f64().unwrap()).unwrap();
            }
        }
    }
    tokens.join(" ")
}
