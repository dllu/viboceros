//! Replay actual keyboard tokens and independently observed pointer context.
pub use super::baseline::{check, check_archive, frame, point, read, setup};
use serde_json::Value;
use viboceros_command::{
    picking::{PointerRay, Projection},
    plane_surface::Preferences,
};
use viboceros_document::Document;
use viboceros_geometry::Vector3;
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

pub fn tokens(doc: &Document, v: &Value) -> Vec<String> {
    let trace = &v["input_trace"][0];
    assert_eq!(trace["completed"], trace["expected"]);
    assert!(trace["errors"].as_array().unwrap().is_empty());
    trace["sent"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let token = s["token"].as_str().unwrap();
            if token.starts_with("'_-SelID ") {
                format!(
                    "Curve={}",
                    doc.objects()
                        .find(|o| o.attributes().name() == Some("guide"))
                        .unwrap()
                        .id()
                )
            } else {
                token.trim_start_matches('_').into()
            }
        })
        .collect()
}
/// The native option prompt records defaults before the first edited setting.
/// These are command input context, independent of surface coefficients.
pub fn preferences(v: &Value) -> Preferences {
    let mut p = Preferences::default();
    for prompt in v["input_trace"][0]["prompts"].as_array().unwrap() {
        let words = prompt
            .as_str()
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>();
        if !words.iter().any(|w| w.starts_with("UDegree=")) {
            continue;
        }
        for (key, value) in words.iter().filter_map(|w| w.split_once('=')) {
            let n: usize = value.parse().unwrap();
            match key {
                "UDegree" => p.degrees[0] = n,
                "VDegree" => p.degrees[1] = n,
                "UPointCount" => p.counts[0] = n,
                "VPointCount" => p.counts[1] = n,
                _ => (),
            }
        }
        break;
    }
    p.validate().unwrap();
    p
}
fn vector(v: &Value) -> Vector3 {
    Vector3::try_from(point(v).to_array()).unwrap()
}
pub fn pointer(v: &Value) -> PointerRay {
    let r = &v["pointer"]["ray"];
    PointerRay::try_new(
        point(&r["origin"]),
        vector(&r["direction"]),
        r["forward_only"].as_bool().unwrap(),
    )
    .unwrap()
}
pub fn projection(v: &Value) -> Projection {
    let c = &v["pointer"]["camera"];
    let view = ThreeDmNamedView {
        name: "Native Plane pointer input".into(),
        camera_location: point(&c["location"]),
        camera_direction: vector(&c["direction"]),
        camera_up: vector(&c["up"]),
        camera_axes: Some(std::array::from_fn(|i| {
            vector(&c["axes"][i]).normalized_nonzero().unwrap()
        })),
        target: Some(point(&c["target"])),
        construction_plane: frame(v),
        projection: if c["projection"] == "Parallel" {
            ThreeDmProjection::Parallel
        } else {
            ThreeDmProjection::Perspective
        },
        frustum: std::array::from_fn(|i| c["frustum"][i].as_f64().unwrap()),
        screen_port: [
            0,
            c["size"][0].as_i64().unwrap() as i32,
            0,
            c["size"][1].as_i64().unwrap() as i32,
        ],
    };
    Projection::from_view(&view)
        .unwrap()
        .with_pointer(Some(pointer(v)))
}
