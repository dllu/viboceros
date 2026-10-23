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
#[test]
fn captured_preselected_spins_use_the_default_native_direction() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/subd_spin_edge.json"
    ))
    .unwrap();
    let mut count = 0;
    for row in capture["results"].as_array().unwrap() {
        let v = &row["value"];
        if v["success"] != true {
            continue;
        }
        let (doc, ids) = native::setup_from_family(row, "subd_spin_edge");
        let Geometry::SubD(s) = doc.object(ids[0]).unwrap().geometry() else {
            panic!()
        };
        let edge = v["before"][0]["selected_edges"][0].as_u64().unwrap() as usize;
        let expected = v["after"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == 0)
            .unwrap();
        let (out, changed) = s.try_spin_edge(edge, false).unwrap();
        assert!(changed);
        let valid = expected["subd"]["valid"] == true;
        let maximum = if valid {
            let check = subd_native::compare(&out, &expected["subd"]);
            Some(
                check["maximum_sampled_surface_error"]
                    .as_f64()
                    .unwrap()
                    .max(
                        check["maximum_control_or_limit_point_error"]
                            .as_f64()
                            .unwrap(),
                    ),
            )
        } else {
            None
        };
        if !valid {
            assert_eq!(out.control_points(), s.control_points());
            assert_eq!(
                out.edges().iter().map(|e| e.to_vec()).collect::<Vec<_>>(),
                expected["subd"]["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| e
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_u64().unwrap() as u32)
                        .collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            );
            viboceros_geometry::SubD::try_from_archive(out.archive()).unwrap();
        }
        count += 1;
        println!(
            "SUBD_SPIN_DISCOVERY {}",
            json!({"id":row["id"],"native_valid":valid,"repaired":!valid,"maximum_error":maximum,"valid_output":true})
        );
    }
    assert_eq!(count, 17);
}
