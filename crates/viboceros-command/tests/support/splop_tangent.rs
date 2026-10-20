//! Tangent reference spheres replay measured native inputs, never inferred outputs.
pub(crate) use crate::splop_guided_test_support as guided;
use serde_json::Value;
use viboceros_command::{
    CommandContext, CommandRegistry, circle_input::CircleDefinition, circle_tangent_input::Input,
};
use viboceros_document::Document;
use viboceros_geometry::{Frame3, Point3, Tolerance, Vector3};

pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/splop_tangent.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/splop_tangent.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 100);
    assert_eq!(r["results"].as_array().unwrap().len(), 100);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            assert!(row["error"].is_null());
            (op.clone(), row["value"].clone())
        })
        .collect()
}
pub fn plane(op: &Value) -> Frame3 {
    let p = &op["construction_plane"];
    Frame3::try_from_directions(
        guided::morph::p(&p[0]),
        Vector3::try_from(guided::morph::p(&p[1]).to_array()).unwrap(),
        Vector3::try_from(guided::morph::p(&p[2]).to_array()).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap()
}
pub fn contact(value: &Value, action: usize) -> Point3 {
    let trace = value["input_trace"]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["action"].as_u64() == Some(action as u64))
        .unwrap();
    assert_eq!(trace["motion_acknowledged"], true);
    assert_eq!(trace["click_acknowledged"], true);
    guided::morph::p(&trace["projected_point"])
}
pub fn reference(
    doc: &Document,
    sources: &guided::Sources,
    op: &Value,
    value: &Value,
) -> Option<CircleDefinition> {
    let mut input = Input::reference_sphere(plane(op), doc.tolerance());
    let mut circle = None;
    for (index, action) in op["actions"].as_array().unwrap().iter().enumerate().skip(1) {
        assert!(circle.is_none());
        circle = if let Some(word) = action.as_str() {
            if word == "Cancel" {
                return None;
            }
            if word == "Enter" {
                input.enter().unwrap()
            } else {
                assert!(input.option(word), "{word}");
                None
            }
        } else if let Some(number) = action.as_f64() {
            input.number(number).unwrap()
        } else if action.is_array() {
            input.point(guided::morph::p(action)).unwrap()
        } else {
            input
                .curve(
                    doc,
                    sources.guides[action["curve"].as_u64().unwrap() as usize],
                    action["edge"].as_u64().map(|x| x as usize),
                    contact(value, index),
                )
                .unwrap()
        };
    }
    circle.map(|c| CircleDefinition::from_circle(c).unwrap())
}
pub fn execute(doc: &mut Document, sources: &guided::Sources, op: &Value, value: &Value) {
    let Some(def) = reference(doc, sources, op, value) else {
        return;
    };
    CommandRegistry::with_builtins()
        .execute_in_context(
            doc,
            &format!(
                "Splop {} {} {} 4 Surface={} {} Sources={} GripSources= Finish={}",
                guided::token(def.frame.origin()),
                def.radius,
                guided::token(sources.target_point),
                sources.target,
                guided::options(op).arguments(),
                sources.source,
                if op["finish"] == "CancelCopies" {
                    "No"
                } else {
                    "Yes"
                }
            ),
            CommandContext {
                construction_plane: def.frame,
            },
        )
        .unwrap();
}

/// Rhino's arc reference axis can amplify a small contact error through the
/// source-to-destination scale. The exact 3-4-5 construction is checked separately.
pub fn check(
    sources: &guided::Sources,
    doc: &Document,
    op: &Value,
    value: &Value,
    phase: &str,
    label: &str,
) -> f64 {
    let bound = if op["curves"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["type"] == "arc")
    {
        2e-5
    } else {
        doc.tolerance().absolute()
    };
    sources.check_with_cloud_tolerance(doc, value, phase, label, bound)
}
