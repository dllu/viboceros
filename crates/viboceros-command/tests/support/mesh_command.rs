//! Exact saved mesh-command geometry, stored channels and document lifecycle.
#[path = "mesh_conversion.rs"]
mod raw_mesh;
use super::subd_native;
use serde_json::{Value, json};
use viboceros_document::{Document, Geometry, ObjectId};

pub fn check(doc: &Document, ids: &[ObjectId], value: &Value, stage: &str) -> f64 {
    let mut maximum = 0f64;
    super::check_metadata(doc, &value[stage], true);
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
                v["normals"] = v["vertex_normals"].clone();
                let (expected, invalid) = raw_mesh::mesh(&v, false);
                assert!(invalid.is_empty());
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
            Geometry::Point(p) => assert_eq!(*p, super::point(&row["point"])),
            _ => panic!(),
        }
    }
    assert!(maximum <= 1e-6);
    maximum
}
