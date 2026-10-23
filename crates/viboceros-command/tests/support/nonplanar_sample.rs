//! Frozen physical getters, precise native options and independent click rays.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_command::triangulate_nonplanar_quads::{self, Options};
use viboceros_geometry::Point3;

pub fn groups() -> Vec<(&'static str, Vec<Value>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/observations").is_dir())
        .unwrap();
    [
        ("nonplanar_sample_shaded", 2),
        ("nonplanar_sample_core", 14),
        ("nonplanar_sample_rules", 5),
        ("nonplanar_sample_external", 1),
        ("nonplanar_sample_raw", 1),
        ("nonplanar_sample_neighbors", 2),
    ]
    .into_iter()
    .map(|(family, count)| {
        let data: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join("tools/rhino_oracle/observations")
                    .join(format!("{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let rows = data["results"].as_array().unwrap();
        assert_eq!(rows.len(), count);
        for row in rows {
            assert!(
                row["value"]["input_trace"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|t| t["errors"].as_array().unwrap().is_empty())
            );
        }
        (family, rows.clone())
    })
    .collect()
}
pub fn native_options(value: &Value, mut before: Options) -> Options {
    for line in value["history"]
        .as_str()
        .unwrap()
        .lines()
        .filter(|l| l.starts_with("Change option or press Enter to accept"))
    {
        let words = line
            .split_whitespace()
            .filter(|s| {
                ["SelectBy=", "Distance=", "Angle=", "SplitMethod="]
                    .iter()
                    .any(|p| s.starts_with(p))
            })
            .map(|s| s.trim_end_matches([')', ':']))
            .collect::<Vec<_>>();
        for word in words {
            let (key, value) = word.split_once('=').unwrap();
            match key {
                "Distance" => before.distance = value.parse().unwrap(),
                "Angle" => before.angle = value.parse().unwrap(),
                _ => before = triangulate_nonplanar_quads::parse(&[word], before).unwrap(),
            }
        }
    }
    before
}
pub fn pointer(value: &Value, action: usize) -> Point3 {
    let trace = &value["input_trace"][0];
    let click = trace["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(action as u64))
        .unwrap();
    assert_eq!(click["motion_acknowledged"], true);
    assert_eq!(click["click_acknowledged"], true);
    assert!(
        trace["prompts"][action]
            .as_str()
            .unwrap()
            .starts_with("Select mesh face")
    );
    let ray: Vec<[f64; 3]> = serde_json::from_value(click["ray"].clone()).unwrap();
    assert_eq!(ray.len(), 2);
    Point3::try_from(std::array::from_fn(|i| ray[0][i] * 0.5 + ray[1][i] * 0.5)).unwrap()
}
pub fn option_errors(a: Options, b: Options) -> (f64, f64) {
    assert_eq!(a.select_by, b.select_by);
    assert_eq!(a.method, b.method);
    let distance = (a.distance - b.distance).abs();
    let angle = (a.angle - b.angle).abs();
    assert!(distance <= 1e-10, "distance {a:?} {b:?}");
    assert!(angle <= 1e-10, "angle {a:?} {b:?}");
    (distance, angle)
}
pub fn report(row: &Value, options: Options, geometry: f64, distance: f64, angle: f64) -> Value {
    let face_picks = row["value"]["input"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a.is_object())
        .count();
    json!({"id":row["id"],"qualified":true,"geometry":true,"channels":true,"identity":true,"metadata":true,"selection":true,"object_order":true,"history":true,
        "staged":true,"raw_face_picker":face_picks>0,"face_picks":face_picks,"accepted":row["value"]["success"],"maximum_error":geometry,"maximum_distance_error":distance,"maximum_angle_error_degrees":angle,
        "preferences":{"select_by":options.select_by.name(),"distance":options.distance,"angle":options.angle,"split_method":options.method_name()}})
}
