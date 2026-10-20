//! Complete native ArrayCrv scenes and observations shared by API and UI replay.
pub use crate::array_curve_surface_test_support::{Scene, plane, point, setup, token};
use serde_json::{Value, json};
use viboceros_command::{CommandContext, CommandRegistry, curve_array::Reference};
use viboceros_document::Document;

pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/curve_array_input.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/curve_array_input.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 62);
    assert_eq!(r["results"].as_array().unwrap().len(), 62);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, r)| {
            assert_eq!(op["id"], r["id"]);
            assert!(r["error"].is_null());
            assert_eq!(r["value"]["input_trace"]["errors"], json!([]));
            (op.clone(), r["value"].clone())
        })
        .collect()
}
pub fn reference(
    doc: &Document,
    scene: &Scene,
    op: &Value,
    value: &Value,
) -> (Reference, Option<[f64; 2]>) {
    let edge = (op["path"] == "Edge").then_some(0);
    let curve =
        viboceros_command::curve_reference::CapturedCurve::capture(doc, scene.rail, edge).unwrap();
    let domain = curve.curve_ref().domain();
    let range = op["subcurve"].as_array().map(|v| {
        [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()]
            .map(|f| domain.start() + f * (domain.end() - domain.start()))
    });
    let pick = if let Some([_, end]) = range {
        Some(curve.curve_ref().evaluate(end).unwrap())
    } else {
        value["input_trace"]["clicks"]
            .as_array()
            .and_then(|v| v.first())
            .map(|v| point(&v["projected_point"]))
    };
    (
        Reference {
            curve: scene.rail,
            edge,
            pick,
        },
        range,
    )
}
pub fn execute(
    doc: &mut Document,
    scene: &Scene,
    commands: &CommandRegistry,
    op: &Value,
    value: &Value,
) {
    let (refs, range) = reference(doc, scene, op, value);
    if op["finish"] != "Accept" {
        if op["postselect"] == true {
            doc.clear_selection();
        }
        return;
    }
    let mut spacing = op["spacing"].as_str().unwrap();
    let mut number = &op["value"];
    let mut orientation = op["orientation"].as_str().unwrap();
    for edit in op["edits"].as_array().unwrap() {
        let (key, v) = edit.as_object().unwrap().iter().next().unwrap();
        if key == "Orientation" {
            orientation = v.as_str().unwrap();
        } else {
            spacing = key;
            number = v;
        }
    }
    let range = range.map_or_else(String::new, |[a, b]| format!(" SubCrv={a},{b}"));
    let base = if op["base_point"].is_null() {
        String::new()
    } else {
        format!(" BasePoint={}", token(point(&op["base_point"])))
    };
    commands
        .execute_in_context(
            doc,
            &format!(
                "ArrayCrv {spacing}={number} {}{range}{base} Orientation={orientation} {}={}",
                refs.arguments(),
                if op["postselect"] == true {
                    "PickedSources"
                } else {
                    "Sources"
                },
                scene
                    .sources
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            CommandContext {
                construction_plane: plane(op),
            },
        )
        .unwrap();
}

/// Keep every capture row in the state replay. Undefined native pole maps and
/// the previously measured native smooth-frame integration error are diagnostics.
pub fn qualification(op: &Value) -> &'static str {
    if matches!(op["path"].as_str(), Some("SkewPolyline" | "SkewPolyline2"))
        && (op["spacing"] == "Distance" || matches!(op["value"].as_u64(), Some(4 | 7)))
    {
        return "native_query_projection_pole";
    }
    if matches!(op["path"].as_str(), Some("LineZ" | "Polyline"))
        && matches!(op["orientation"].as_str(), Some("Roadlike" | "Stairlike"))
    {
        "native_vertical_degeneracy"
    } else if op["path"] == "ClosedNurbs" && op["orientation"] == "Freeform" {
        "native_frame_approximation"
    } else {
        "qualified"
    }
}
pub fn check(scene: &Scene, doc: &Document, op: &Value, value: &Value, phase: &str) -> f64 {
    let id = op["id"].as_str().unwrap();
    let max = scene.audit(doc, value, phase, id);
    match qualification(op) {
        "native_vertical_degeneracy" | "native_query_projection_pole" => {}
        "native_frame_approximation" => {
            assert!(max <= 5e-5, "{id} {phase}: native frame discrepancy {max}")
        }
        _ => assert!(max <= 1e-6, "{id} {phase}: cloud error {max}"),
    }
    max
}

pub fn frame_rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/curve_array_frames.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/curve_array_frames.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 44);
    assert_eq!(r["results"].as_array().unwrap().len(), 44);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            assert!(row["error"].is_null());
            let v = &row["value"];
            assert_eq!(v["input_trace"]["errors"], json!([]));
            (op.clone(), v.clone())
        })
        .collect()
}
pub fn stable_frame_rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/curve_array_frames_stable.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/curve_array_frames_stable.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 16);
    assert_eq!(r["results"].as_array().unwrap().len(), 16);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            assert!(row["error"].is_null());
            let v = &row["value"];
            assert_eq!(v["input_trace"]["errors"], json!([]));
            (op.clone(), v.clone())
        })
        .collect()
}
