//! Replays physical native SubD point witnesses against the kernel.
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
#[allow(dead_code)]
#[path = "support/crease.rs"]
mod native;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use serde_json::{Value, json};
use viboceros_document::Geometry;
use viboceros_geometry::{LineSegment, PolygonEdgePoint, Tolerance, Vector3};

fn picks(geometry: &Geometry, v: &Value, tolerance: Tolerance) -> Vec<PolygonEdgePoint> {
    let lines = match geometry {
        Geometry::Mesh(m) => m.wireframe_lines(tolerance).unwrap(),
        Geometry::SubD(s) => s
            .edges()
            .iter()
            .map(|&[a, b]| {
                LineSegment::try_new(
                    s.control_points()[a as usize],
                    s.control_points()[b as usize],
                    tolerance,
                )
                .unwrap()
            })
            .collect(),
        _ => panic!(),
    };
    let mut out = Vec::new();
    for (index, action) in v["input"]["actions"].as_array().unwrap().iter().enumerate() {
        if action.is_object() {
            let witness = v["input_trace"][0]["clicks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["action"] == index)
                .unwrap();
            let point = native::point(&witness["projected_point"]);
            let (_, pick) = lines
                .iter()
                .enumerate()
                .map(|(edge, line)| {
                    let parameter = line.closest_parameter(point, tolerance).unwrap();
                    let distance = line
                        .point_at(parameter)
                        .unwrap()
                        .distance_to(point)
                        .unwrap();
                    (distance, PolygonEdgePoint { edge, parameter })
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .unwrap();
            out.push(pick);
        } else {
            match action.as_str().unwrap() {
                "Undo" => {
                    out.pop();
                }
                "Cancel" => out.clear(),
                "Enter" => {}
                _ => panic!(),
            }
        }
    }
    out
}
fn replay(id: &str) {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/insert_point_full.json"
    ))
    .unwrap();
    let mut count = 0;
    for row in capture["results"].as_array().unwrap() {
        if row["id"] != id {
            continue;
        }
        eprintln!("Replaying {}", row["id"]);
        let v = &row["value"];
        let (doc, ids) = native::setup_from_family(row, "insert_point_full");
        let source = doc.object(ids[0]).unwrap().geometry();
        let picks = picks(source, v, doc.tolerance());
        let expected = v["after"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == 0)
            .unwrap();
        let normal = Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let report = match source {
            Geometry::SubD(s) => {
                let actual = s
                    .try_insert_edge_path(&picks, Some(normal), doc.tolerance())
                    .unwrap();
                assert_eq!(
                    actual.control_points().len(),
                    expected["subd"]["points"].as_array().unwrap().len()
                );
                for (a, b) in actual
                    .control_points()
                    .iter()
                    .zip(expected["subd"]["points"].as_array().unwrap())
                {
                    assert!(a.distance_to(native::point(b)).unwrap() <= 1e-6);
                }
                subd_native::compare(&actual, &expected["subd"])
            }
            _ => panic!(),
        };
        println!(
            "INSERT_POINT_KERNEL {}",
            json!({"id":row["id"],"comparison":report})
        );
        count += 1;
    }
    assert_eq!(count, 1);
}
macro_rules! cases {
    ($($name:ident),* $(,)?)=>{$(#[test]fn $name(){replay(concat!("insert_point_full_",stringify!($name)));})*};
}
cases!(
    subd_adjacent,
    subd_three,
    subd_same_edge,
    subd_diagonal,
    subd_smooth,
    subd_strip_mid,
    subd_strip_cross,
    subd_strip_skip,
    subd_undo_all,
    subd_cancel_empty
);
