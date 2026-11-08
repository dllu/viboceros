//! Authored Shell sources and finite native document/history comparisons.
use super::brep_comparison::{captured, geometry};
use super::native;
use serde_json::{Value, json};
use viboceros_command::{CommandContext, CommandRegistry, shell::ShellOptions};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{Brep, Point3, Tolerance};
#[path = "shell/check.rs"]
mod comparison;
pub use comparison::{State, check};

pub fn rows() -> Vec<(String, Value)> {
    [
        "shell_getter_pilot",
        "shell_getter_qualified",
        "shell_getter_holdouts",
    ]
    .into_iter()
    .flat_map(|family| {
        native::read(family)["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (row["id"].as_str().unwrap().to_owned(), row["value"].clone()))
            .collect::<Vec<_>>()
    })
    .collect()
}
pub fn success(v: &Value) -> bool {
    v["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "Shell")
        .unwrap()["result"]
        == "Success"
}
pub fn diagnostic(v: &Value) -> bool {
    v["input_trace"][0]["errors"] != json!([])
        || v["history"].as_str().unwrap().contains("Unknown command:")
}
pub fn guide(doc: &Document, index: usize) -> ObjectId {
    let name = format!("guide{index}");
    doc.objects()
        .find(|o| o.attributes().name() == Some(name.as_str()))
        .unwrap()
        .id()
}
pub fn setup(v: &Value) -> Document {
    let t = &v["model_tolerance"];
    let tolerance = Tolerance::try_new(
        t["absolute"].as_f64().unwrap(),
        t["relative"].as_f64().unwrap(),
        t["angular"].as_f64().unwrap(),
    )
    .unwrap();
    let mut d = native::scene_with_geometry(v, |row| {
        if row["source"] == true {
            return Geometry::Point(Point3::try_new(50., 60., 70.).unwrap());
        }
        let i = row["attributes"]["name"]
            .as_str()
            .unwrap()
            .strip_prefix("guide")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let source = &v["prescribed_case"]["curves"][i];
        source_geometry(source, tolerance)
    });
    d.set_tolerance(tolerance);
    d.clear_history().unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut d, "Point 90,91,92")
        .unwrap();
    d.undo().unwrap();
    d
}
pub fn face(doc: &Document, object: ObjectId, xyz: &Value) -> usize {
    let Geometry::Brep(b) = doc.object(object).unwrap().geometry() else {
        panic!("shell source")
    };
    let p = native::point(xyz);
    let t = doc.tolerance();
    let found = b
        .faces()
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            let (u, v) = f.surface().closest_parameters(p, t).unwrap();
            (f.contains_parameters(u, v, t).unwrap()
                && f.surface().evaluate(u, v).unwrap().distance_to(p).unwrap() <= t.absolute())
            .then_some(i)
        })
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1);
    found[0]
}
pub fn initial(v: &Value) -> ShellOptions {
    let p = v["input_trace"][0]["prompts"][0].as_str().unwrap();
    let value = |key: &str| {
        p.split(&format!("{key}="))
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
    };
    ShellOptions {
        thickness: value("Thickness").parse().unwrap(),
        delete_input: value("DeleteInput") == "Yes",
    }
}
pub fn command(doc: &Document, v: &Value) -> String {
    let mut options = initial(v);
    let mut faces = Vec::new();
    for a in v["prescribed_case"]["actions"].as_array().unwrap() {
        if let Some(p) = a.get("click") {
            let (id, index) = picked_face(doc, p);
            faces.push(format!("{id}:{index}"));
            continue;
        }
        if let Some(n) = a.as_f64() {
            if n >= 0. {
                options.thickness = n;
            }
            continue;
        }
        let text = a
            .get("number")
            .and_then(Value::as_str)
            .or_else(|| a.as_str());
        if let Some(text) = text {
            if text.starts_with("DeleteInput=") {
                options.delete_input = text == "DeleteInput=Yes";
            }
            if let Some(value) = a.get("number").and_then(Value::as_str) {
                options.thickness =
                    viboceros_drafting::PointInput::parse_length_with_units(value, doc.units())
                        .unwrap()
                        .unwrap();
            }
        }
    }
    format!(
        "Shell {} FaceRefs={}",
        options.command_options(),
        faces.join(",")
    )
}

pub fn picked_face(doc: &Document, xyz: &Value) -> (ObjectId, usize) {
    let p = native::point(xyz);
    let t = doc.tolerance();
    let found = doc
        .objects()
        .filter(|o| {
            o.attributes()
                .name()
                .is_some_and(|n| n.starts_with("guide"))
        })
        .filter_map(|o| {
            let Geometry::Brep(b) = o.geometry() else {
                return None;
            };
            let (index, u, v) = b.closest_face_parameters(p, t).unwrap()?;
            (b.faces()[index]
                .surface()
                .evaluate(u, v)
                .unwrap()
                .distance_to(p)
                .unwrap()
                <= t.absolute())
            .then_some((o.id(), index))
        })
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1);
    found[0]
}
pub fn supported(doc: &Document, v: &Value) -> bool {
    cfg!(feature = "native-smlib")
        || !success(v)
        || viboceros_command::shell::ShellInput::parse(
            &command(doc, v)
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .options
        .thickness
            == 0.
}

pub fn source_geometry(source: &Value, tolerance: Tolerance) -> Geometry {
    match source["type"].as_str().unwrap() {
        "box_brep" => {
            let lo = native::point(&source["min"]).to_array();
            let hi = native::point(&source["max"]).to_array();
            Geometry::Brep(
                Brep::try_box(
                    CommandContext::default().construction_plane,
                    [[lo[0], hi[0]], [lo[1], hi[1]], [lo[2], hi[2]]],
                    tolerance,
                )
                .unwrap(),
            )
        }
        "cylinder_brep" => {
            let frame = viboceros_geometry::Frame3::try_from_normal(
                native::point(&source["origin"]),
                viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
                tolerance,
            )
            .unwrap();
            Geometry::Brep(
                Brep::try_cylinder(
                    frame,
                    source["radius"].as_f64().unwrap(),
                    0.,
                    source["height"].as_f64().unwrap(),
                    tolerance,
                )
                .unwrap(),
            )
        }
        other => panic!("unexpected authored Shell source {other}"),
    }
}
pub fn sdk_error(actual: &Brep, expected: &Value, tolerance: Tolerance) -> f64 {
    geometry::error(actual, &json!({"brep":expected}), tolerance)
}
pub fn sdk_set_error(actual: &Brep, expected: &Value, tolerance: Tolerance) -> f64 {
    geometry::set_error(actual, &json!({"brep":expected}), tolerance)
}
pub fn native_brep(record: &Value, tolerance: Tolerance) -> Brep {
    captured::expected(record, tolerance)
}
