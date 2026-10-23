#[allow(dead_code)]
#[path = "support/crease.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use serde_json::{Value, json};
use viboceros_document::Geometry;

#[test]
fn native_subd_edge_seams_retain_control_topology_tags_and_limit_surfaces() {
    let mut failures = Vec::new();
    let mut count = 0;
    for (family, capture) in [
        (
            "component_seams_subd",
            include_str!("../../../tools/rhino_oracle/observations/component_seams_subd.json"),
        ),
        (
            "component_seams_subd_edges",
            include_str!(
                "../../../tools/rhino_oracle/observations/component_seams_subd_edges.json"
            ),
        ),
        (
            "component_seams_subd_edge_holdouts",
            include_str!(
                "../../../tools/rhino_oracle/observations/component_seams_subd_edge_holdouts.json"
            ),
        ),
        (
            "component_seams_subd_edge_contacts",
            include_str!(
                "../../../tools/rhino_oracle/observations/component_seams_subd_edge_contacts.json"
            ),
        ),
        (
            "component_seams_subd_edge_tolerances_v2",
            include_str!(
                "../../../tools/rhino_oracle/observations/component_seams_subd_edge_tolerances_v2.json"
            ),
        ),
    ] {
        let capture: Value = serde_json::from_str(capture).unwrap();
        for row in capture["results"].as_array().unwrap() {
            let value = &row["value"];
            let command = value["input"]["command"].as_str().unwrap();
            if !matches!(command, "WeldEdge" | "UnweldEdge") {
                continue;
            }
            let (doc, ids) = fixture::setup_from_family(row, family);
            for before in value["before"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s.get("subd").is_some())
            {
                count += 1;
                let source = before["source"].as_u64().unwrap() as usize;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let Geometry::SubD(subd) = doc.object(ids[source]).unwrap().geometry() else {
                        panic!()
                    };
                    let edges = before["selected_edges"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| i.as_u64().unwrap() as usize)
                        .collect::<Vec<_>>();
                    let (edited, changed) = if command == "WeldEdge" {
                        subd.try_weld_edges(&edges)
                    } else {
                        subd.try_unweld_edges(&edges)
                    }
                    .unwrap();
                    let after = value["after"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|s| s["source"] == before["source"])
                        .unwrap();
                    let expected = &after["subd"];
                    let mapping = subd_native::control_map_with_face_incidence(&edited, expected);
                    let raw_ordinals = mapping.iter().enumerate().all(|(i, &j)| i == j);
                    let edge_ordinals = edited
                        .edges()
                        .iter()
                        .zip(expected["edges"].as_array().unwrap())
                        .all(|(&[a, b], native)| {
                            let mut actual = [mapping[a as usize], mapping[b as usize]];
                            let mut expected = [
                                native[0].as_u64().unwrap() as usize,
                                native[1].as_u64().unwrap() as usize,
                            ];
                            actual.sort();
                            expected.sort();
                            actual == expected
                        });
                    let check = subd_native::compare_with_control_map(&edited, expected, &mapping);
                    assert_eq!(changed > 0, before["crc"] != after["crc"]);
                    println!(
                        "SUBD_EDGE_GEOMETRY {}",
                        json!({"id":row["id"],"source":source,"maximum_error":check["maximum_sampled_surface_error"].as_f64().unwrap().max(check["maximum_control_or_limit_point_error"].as_f64().unwrap()),"changed_edges":changed,"qualified":true,"raw_control_ordinals_match":raw_ordinals,"edge_ordinals_match":edge_ordinals})
                    );
                }));
                if result.is_err() {
                    failures.push(format!("{}:{source}", row["id"]));
                }
            }
        }
    }
    assert_eq!(count, 94);
    assert!(failures.is_empty(), "SubD seam differences: {failures:?}");
}
