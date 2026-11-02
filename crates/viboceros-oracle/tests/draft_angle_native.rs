use serde_json::json;
use viboceros_geometry::{DraftAngleMapping, Vector3};
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "support/direction_analysis.rs"]
#[allow(dead_code)]
mod compare_support;
#[path = "support/draft_angle.rs"]
mod support;

#[test]
fn command_settings_selection_and_lifecycle_match_native() {
    for (op, native) in support::cases() {
        let request:ProbeRequest=serde_json::from_value(json!({"protocol_version":1,"iterations":1,"tolerance":{"absolute":1e-8,"relative":1e-12,"angular":1e-8},"operations":[op]})).unwrap();
        let actual = run_request(&request).unwrap();
        compare_support::compare(
            &actual.results[0].value,
            &support::expected(&op, &native),
            op["id"].as_str().unwrap(),
        );
    }
}

#[test]
fn native_mesh_normal_colors_match_exactly_including_singleton_and_reverse_ranges() {
    let mut count = 0;
    for (op, native) in support::cases() {
        for t in native["traces"].as_array().unwrap() {
            let s = &t["settings"];
            let up: [f64; 3] = serde_json::from_value(s["up"].clone()).unwrap();
            let range: [f64; 2] = serde_json::from_value(s["range"].clone()).unwrap();
            let mapping = DraftAngleMapping::try_new(
                Vector3::try_new(up[0], up[1], up[2]).unwrap(),
                range.map(f64::to_degrees),
            )
            .unwrap();
            for m in t["meshes"].as_array().unwrap() {
                for (n, c) in m["normals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(m["colors"].as_array().unwrap())
                {
                    let n: [f64; 3] = serde_json::from_value(n.clone()).unwrap();
                    let c: [u8; 4] = serde_json::from_value(c.clone()).unwrap();
                    assert_eq!(
                        mapping.color(Vector3::try_new(n[0], n[1], n[2]).unwrap()),
                        [c[0], c[1], c[2]],
                        "{} normal {n:?}",
                        op["id"]
                    );
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 10_160);
}
