//! Display inputs and public curvature observations remain independent of outputs.
use super::{edges, native, reference_inputs};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use viboceros_command::{CommandRegistry, blend_curve::Input, curvature_graph::Options};
use viboceros_document::{Document, SelectionMode};

pub const FAMILIES: [&str; 5] = [
    "blend_curve_display_discovery",
    "blend_curve_display_lifecycle",
    "blend_curve_display_verification",
    "blend_curve_display_defaults",
    "blend_curve_display_control_defaults",
];
pub const CASES: usize = 26;

pub fn setup(registry: &CommandRegistry, value: &Value) -> Document {
    let mut doc = edges::setup(value);
    let settings = &value["prescribed_case"]["graph_settings"];
    let mut options = Options::default();
    if !settings.is_null() {
        options.scale = settings["scale"].as_u64().unwrap() as u16;
        options.density = settings["density"].as_u64().unwrap() as u8;
    }
    registry.configure_curvature_graph(&doc, options).unwrap();
    doc.clear_selection();
    if let Some(indices) = value["prescribed_case"]["initial_graphs"].as_array() {
        // Native setup enables analysis by object ID, without group expansion.
        doc.select_objects_direct(
            indices
                .iter()
                .map(|i| reference_inputs::guide(&doc, i.as_u64().unwrap() as usize))
                .collect::<Vec<_>>(),
            SelectionMode::Add,
        )
        .unwrap();
        registry.execute(&mut doc, "CurvatureGraph").unwrap();
        doc.clear_selection();
    }
    doc
}
#[allow(dead_code)]
pub fn input(doc: &Document, registry: &CommandRegistry, value: &Value) -> Input {
    let mut input = Input::with_preferences(doc.tolerance(), edges::preferences(value));
    input.inherit_curvature_display(
        registry
            .curvature_graph_view(doc)
            .unwrap()
            .is_some_and(|v| v.sources > 0),
    );
    input
}
fn graph_names(doc: &Document, registry: &CommandRegistry) -> BTreeSet<String> {
    registry
        .curvature_graph_view(doc)
        .unwrap()
        .into_iter()
        .flat_map(|v| v.graphs.iter().map(|g| g.object).collect::<Vec<_>>())
        .map(|id| {
            doc.object(id)
                .unwrap()
                .attributes()
                .name()
                .unwrap_or("")
                .to_owned()
        })
        .collect()
}
pub fn snapshot(
    input: &Input,
    doc: &Document,
    registry: &CommandRegistry,
    value: &Value,
    index: usize,
) {
    let shot = value["input_trace"][0]["viewport_snapshots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["action"].as_u64() == Some(index as u64))
        .unwrap();
    let options = registry.curvature_graph_options();
    assert_eq!(
        u64::from(options.scale),
        shot["settings"]["scale"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(options.density),
        shot["settings"]["density"].as_u64().unwrap()
    );
    let mut names = graph_names(doc, registry);
    if let Some(display) = input
        .curvature_display(&registry.curvature_graph_options())
        .unwrap()
    {
        assert!(!display.data.blend.hairs.is_empty());
        for id in &display.data.objects {
            names.insert(
                doc.object(*id)
                    .unwrap()
                    .attributes()
                    .name()
                    .unwrap_or("")
                    .to_owned(),
            );
        }
    }
    if let Some(objects) = shot["object_graphs"].as_array() {
        let expected = objects
            .iter()
            .filter(|o| o["enabled"] == true)
            .map(|o| o["name"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(names, expected, "display analysis at action {index}");
    }
}
pub fn completed(doc: &Document, registry: &CommandRegistry, value: &Value) {
    let shots = value["completion_observation"].as_array();
    if let Some(shots) = shots {
        for shot in shots {
            let expected = shot["object_graphs"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["enabled"] == true)
                .map(|o| o["name"].as_str().unwrap().to_owned())
                .collect::<BTreeSet<_>>();
            assert_eq!(graph_names(doc, registry), expected, "completed analysis");
        }
    }
}
pub fn curvature(doc: &Document, target: &Value, options: &Options) -> (f64, f64, usize) {
    let mut vector = 0_f64;
    let mut tip = 0_f64;
    let mut count = 0;
    for row in target.as_array().unwrap() {
        let Some(samples) = row["curvature_samples"].as_array() else {
            continue;
        };
        let c = doc
            .objects()
            .find(|o| {
                o.attributes().name().unwrap_or("") == row["attributes"]["name"].as_str().unwrap()
            })
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        for sample in samples {
            let t = c
                .parameter_at(sample["fraction"].as_f64().unwrap())
                .unwrap();
            let actual = c.curvature_vector(t).unwrap();
            let expected = native::point(&sample["curvature"]).to_array();
            let actual_values = actual.to_array();
            vector = vector.max(
                (0..3)
                    .map(|i| (actual_values[i] - expected[i]).abs())
                    .fold(0., f64::max),
            );
            let actual_tip = c
                .evaluate(t)
                .unwrap()
                .translated(actual.scaled(-options.factor()).unwrap())
                .unwrap();
            let point = native::point(&sample["point"]).to_array();
            let expected_tip = viboceros_geometry::Point3::try_from(std::array::from_fn(|i| {
                point[i] - expected[i] * options.factor()
            }))
            .unwrap();
            tip = tip.max(actual_tip.distance_to(expected_tip).unwrap());
            count += 1;
        }
    }
    assert!(
        vector <= 1e-6 && tip <= 1e-6,
        "curvature={vector:e}, tips={tip:e}"
    );
    (vector, tip, count)
}
pub fn measurements(
    before: (f64, f64, usize),
    after: (f64, f64, usize),
    mut metrics: Value,
) -> Value {
    metrics["maximum_curvature_vector_error"] = json!(before.0.max(after.0));
    metrics["maximum_scaled_curvature_tip_error"] = json!(before.1.max(after.1));
    metrics["curvature_stations"] = json!(before.2 + after.2);
    metrics
}
