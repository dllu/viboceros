use super::subd_native;
use serde_json::{Value, json};
use viboceros_document::{Document, Geometry, GeometrySnapshot, ObjectId};
use viboceros_geometry::{Frame3, LineSegment, Point3, Tolerance, Vector3};
pub struct Scene {
    pub document: Document,
    pub source: Option<ObjectId>,
    pub original: Option<GeometrySnapshot>,
}
pub fn rows() -> Vec<Value> {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/subd_box.json"
    ))
    .unwrap();
    let rows = data["results"].as_array().unwrap();
    assert_eq!(rows.len(), 23);
    rows.clone()
}
pub fn point(p: &Value) -> Point3 {
    Point3::try_new(
        p[0].as_f64().unwrap(),
        p[1].as_f64().unwrap(),
        p[2].as_f64().unwrap(),
    )
    .unwrap()
}
pub fn plane(value: &Value) -> Frame3 {
    if let Some(v) = value["input"].get("construction_plane") {
        Frame3::try_from_directions(
            point(&v[0]),
            Vector3::try_from(point(&v[1]).to_array()).unwrap(),
            Vector3::try_from(point(&v[2]).to_array()).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    } else {
        viboceros_command::CommandContext::default().construction_plane
    }
}
pub fn setup(value: &Value) -> Scene {
    setup_on_layer(value, "SubDBox output")
}
pub fn setup_on_layer(value: &Value, name: &str) -> Scene {
    let mut document = Document::default();
    let layer = document
        .add_layer(name, viboceros_document::ColorRgb::new(0, 0, 255))
        .unwrap();
    document.set_current_layer(layer).unwrap();
    let source = value["input"].get("curve").map(|c| {
        document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(point(&c[0]), point(&c[1]), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap()
    });
    let original = source.map(|id| document.object(id).unwrap().geometry_snapshot().clone());
    document.clear_history().unwrap();
    Scene {
        document,
        source,
        original,
    }
}
pub fn header(value: &Value) -> String {
    let counts = value["input"]
        .get("counts")
        .map(|c| format!(" XCount={} YCount={} ZCount={}", c[0], c[1], c[2]))
        .unwrap_or_default();
    format!("SubDBox{counts}")
}
pub fn actions(value: &Value, source: Option<ObjectId>) -> Vec<String> {
    value["input"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            if a.is_object() {
                format!("Curve={}", source.unwrap())
            } else if let Some(s) = a.as_str() {
                s.into()
            } else if let Some(p) = a.as_array() {
                p.iter()
                    .map(|v| v.as_f64().unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            } else {
                a.to_string()
            }
        })
        .collect()
}
#[allow(dead_code)]
pub fn command(value: &Value, source: Option<ObjectId>) -> String {
    format!("{} {}", header(value), actions(value, source).join(" "))
}
pub fn check(scene: &Scene, value: &Value, phase: &str) -> Value {
    let targets = value[phase].as_array().unwrap();
    assert_eq!(scene.document.objects().count(), targets.len());
    if let Some(id) = scene.source {
        assert!(
            scene
                .original
                .as_ref()
                .unwrap()
                .shares_storage_with(scene.document.object(id).unwrap().geometry_snapshot())
        );
    }
    let mut objects = Vec::new();
    for target in targets {
        let object = if target["role"] == "source" {
            scene.document.object(scene.source.unwrap()).unwrap()
        } else {
            scene
                .document
                .objects()
                .find(|o| Some(o.id()) != scene.source)
                .unwrap()
        };
        assert_eq!(
            scene.document.is_selected(object.id()),
            target["selected"].as_bool().unwrap()
        );
        assert_eq!(object.attributes().name(), target["name"].as_str());
        assert_eq!(
            scene
                .document
                .layer(object.attributes().layer_id())
                .unwrap()
                .name(),
            target["layer"].as_str().unwrap()
        );
        if let Some(expected) = target.get("subd") {
            let Geometry::SubD(subd) = object.geometry() else {
                panic!("expected SubD")
            };
            objects.push(subd_native::compare(subd, expected));
        }
    }
    json!({"qualified":true,"objects":objects,"attributes_selection_and_source":true})
}
