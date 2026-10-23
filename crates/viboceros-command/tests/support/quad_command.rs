//! Exact native command geometry, channels, object order and lifecycle fixtures.
use super::{mesh_native, native, subd_native};
use serde_json::{Value, json};
use viboceros_command::quadrangulate_mesh::Options;
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::Vector3;

pub fn groups() -> Vec<(&'static str, Vec<Value>)> {
    [
        (
            "quad_command_discovery",
            include_str!("../../../../tools/rhino_oracle/observations/quad_command_discovery.json"),
        ),
        (
            "quad_command_main",
            include_str!("../../../../tools/rhino_oracle/observations/quad_command_main.json"),
        ),
    ]
    .into_iter()
    .map(|(family, text)| {
        (
            family,
            serde_json::from_str::<Value>(text).unwrap()["results"]
                .as_array()
                .unwrap()
                .clone(),
        )
    })
    .collect()
}
pub fn arguments(value: &Value) -> String {
    value["input"]["command_arguments"]
        .as_str()
        .unwrap()
        .split_whitespace()
        .filter(|w| !["_All", "_Enter"].contains(w))
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn command(value: &Value) -> String {
    format!("QuadrangulateMesh {}", arguments(value))
}
pub fn check(doc: &Document, ids: &[ObjectId], value: &Value, stage: &str) -> f64 {
    let mut maximum = 0f64;
    native::check_metadata(doc, &value[stage], true);
    for (object, row) in doc.objects().zip(value[stage].as_array().unwrap()) {
        let color = object.attributes().object_color();
        assert_eq!(json!([color.red, color.green, color.blue]), row["color"]);
        assert_eq!(
            object.attributes().color_source() as u8,
            row["color_source"].as_u64().unwrap() as u8
        );
        if let Some(source) = row["source"].as_u64() {
            assert_eq!(object.id(), ids[source as usize]);
        }
        match object.geometry() {
            Geometry::Mesh(actual) => {
                let mut v = row["mesh"].clone();
                if let Some(colors) = v["colors"].as_array_mut() {
                    for c in colors {
                        c[3] = json!(255 - c[3].as_u64().unwrap());
                    }
                }
                let mut expected = mesh_native::mesh(&v);
                if let Some(uv) = v["uv"].as_array().filter(|a| !a.is_empty()) {
                    expected = expected
                        .try_with_texture_coordinates(Some(
                            serde_json::from_value(Value::Array(uv.clone())).unwrap(),
                        ))
                        .unwrap();
                }
                if let Some(normals) = v["vertex_normals"].as_array().filter(|a| !a.is_empty()) {
                    expected = expected
                        .try_with_vertex_normals(Some(
                            normals
                                .iter()
                                .map(|n| {
                                    Vector3::try_from(
                                        serde_json::from_value::<[f64; 3]>(n.clone()).unwrap(),
                                    )
                                    .unwrap()
                                })
                                .collect(),
                        ))
                        .unwrap();
                }
                assert_eq!(actual, &expected, "raw mesh/channels at {stage}");
            }
            Geometry::SubD(s) => {
                let report = subd_native::compare(s, &row["subd"]);
                assert_eq!(report["qualified"], true);
                maximum = maximum
                    .max(
                        report["maximum_control_or_limit_point_error"]
                            .as_f64()
                            .unwrap(),
                    )
                    .max(report["maximum_sampled_surface_error"].as_f64().unwrap());
            }
            Geometry::Point(p) => assert_eq!(*p, native::point(&row["point"])),
            _ => panic!(),
        }
    }
    assert!(maximum <= 1e-6);
    maximum
}
pub fn report(row: &Value, options: Options, maximum: f64) -> Value {
    json!({"id":row["id"],"qualified":true,"geometry":true,"channels":true,"identity":true,
        "metadata":true,"object_order":true,"selection":true,"history":true,"maximum_error":maximum,
        "accepted":row["value"]["success"],"preferences":{"planarity":options.planarity,"rectangularity":options.rectangularity}})
}
