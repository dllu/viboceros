//! Full image bounds and owned physical FitSrf observations remain distinct.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;

#[test]
fn complete_image_proofs_cover_every_planar_fit_capture() {
    let mut cases = 0;
    let mut planar = 0;
    let mut nonplanar = 0;
    let mut changed_parameters = 0;
    for family in ["surface_fit", "surface_fit_adaptive", "surface_fit_strips"] {
        let native = captured::read(family);
        for row in native["results"].as_array().unwrap() {
            let value = &row["value"];
            assert_eq!(value["before"], value["after"]);
            if value["surface"].is_null() {
                continue;
            }
            let a = captured::surface(&value["before"]);
            let b = captured::surface(&value["surface"]);
            let originals = (a.clone(), b.clone());
            let bound = a.planar_surface_image_deviation_bound(&b, 1e-9).unwrap();
            let flat = a.control_points().iter().all(|p| p.point().z() == 0.)
                && b.control_points().iter().all(|p| p.point().z() == 0.);
            let parameter_bound = if flat {
                a.parameter_surface_deviation_bound(&b, 1e-9).unwrap()
            } else {
                None
            };
            if flat {
                assert!(bound.is_some(), "{}: complete planar image", row["id"]);
                planar += 1;
                changed_parameters += usize::from(parameter_bound.is_none());
            } else {
                assert!(
                    bound.is_none(),
                    "{}: unsupported narrow quad enclosure",
                    row["id"]
                );
                nonplanar += 1;
            }
            assert_eq!((a, b), originals);
            cases += 1;
            println!(
                "PLANAR_FIT_IMAGE {}",
                json!({"id":row["id"],"planar_fixture":flat,"image_bound":bound,"parameter_bound":parameter_bound})
            );
        }
    }
    assert_eq!((cases, planar, nonplanar), (90, 30, 60));
    assert!(changed_parameters > 0);
    println!(
        "PLANAR_FIT_IMAGE_TOTAL {}",
        json!({"cases":cases,"certified_planar_images":planar,"parameter_unproved":changed_parameters})
    );
}

#[test]
fn physical_fit_commands_keep_selection_eligibility_metadata_and_history_explicit() {
    let mut data = Vec::new();
    for (family, count) in [
        ("surface_fit_commands", 8),
        ("surface_fit_command_faces", 7),
    ] {
        let query: Value = serde_json::from_slice(
            &std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../../tools/rhino_oracle/fixtures/{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let native = captured::read(family);
        assert_eq!(query["operations"].as_array().unwrap().len(), count);
        assert_eq!(native["results"].as_array().unwrap().len(), count);
        data.extend(
            query["operations"]
                .as_array()
                .unwrap()
                .iter()
                .cloned()
                .zip(native["results"].as_array().unwrap().iter().cloned()),
        );
    }
    let mut accepted = 0;
    let mut rejected = 0;
    let mut declined = 0;
    for (op, row) in data {
        assert_eq!(op["id"], row["id"]);
        let v = &row["value"];
        let c = &v["command"];
        assert_eq!(c["completed"], 1);
        assert_eq!(c["active"], false);
        assert_eq!(c["after"], c["after_cancel"]);
        assert_eq!(v["undo"]["success"], true);
        assert_eq!(v["redo"]["success"], true);
        assert_eq!(v["undo"]["after"], v["before"]);
        assert_eq!(v["redo"]["after"], c["after"]);
        let source = &v["before"][0];
        assert_eq!(source["source"], 0);
        for object in c["after"].as_array().unwrap() {
            assert_eq!(object["valid"], true);
            assert!(
                object["untrimmed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|v| v == true)
            );
        }
        let faces = source["definitions"].as_array().unwrap().len();
        let mut bound = None;
        let reason;
        if faces > 1 {
            assert_eq!(faces, 4);
            assert_eq!(c["success"], false);
            assert_eq!(c["after"], v["before"]);
            rejected += 1;
            reason = "selection_rejected";
        } else if v["sdk"].is_null() {
            assert_eq!(c["success"], false);
            assert_eq!(c["after"], v["before"]);
            assert!(
                c["history"]
                    .as_str()
                    .unwrap()
                    .contains("could not be simplified")
            );
            declined += 1;
            reason = "fit_declined";
        } else {
            assert_eq!(c["success"], true);
            assert!(!v["sdk"].is_null());
            accepted += 1;
            reason = "accepted";
            let delete = op["delete_input"] == true;
            let current = op["output_layer"] == "current";
            let after = c["after"].as_array().unwrap();
            assert_eq!(after.len(), if delete { 1 } else { 2 });
            if !delete {
                assert_eq!(after[0], *source);
            }
            let result = &after[usize::from(!delete)];
            assert_eq!(
                result["source"],
                if delete { json!(0) } else { Value::Null }
            );
            assert_eq!(result["layer"], usize::from(current));
            assert_eq!(
                result["groups"],
                if current && !delete {
                    json!([])
                } else {
                    json!([0])
                }
            );
            assert_eq!(result["selected"], delete && op["preselect"] == true);
            assert_eq!(
                result["name"],
                if current && !delete {
                    Value::Null
                } else {
                    json!("source")
                }
            );
            assert_eq!(
                result["attribute_text"],
                if current && !delete {
                    Value::Null
                } else {
                    json!("attribute")
                }
            );
            assert_eq!(result["definition"], v["sdk"]);
            assert_eq!(
                result["color_source"],
                if current && !delete {
                    json!("ColorFromLayer")
                } else {
                    json!("ColorFromObject")
                }
            );
            if delete || !current {
                assert_eq!(result["color"], json!([20, 40, 60]));
            }
            let actual = captured::surface(&result["definition"]);
            let sdk = captured::surface(&v["sdk"]);
            bound = Some(
                actual
                    .parameter_surface_deviation_bound(&sdk, 1e-9)
                    .unwrap()
                    .unwrap(),
            );
        }
        println!(
            "PHYSICAL_SURFACE_FIT {}",
            json!({"id":op["id"],"accepted":c["success"],"reason":reason,"input_faces":faces,"sdk_bound":bound,"undo_redo":true})
        );
    }
    assert_eq!((accepted, rejected, declined), (9, 3, 3));
}

fn definition(s: &NurbsSurface) -> Value {
    json!({"degree_u":s.degree_u(),"degree_v":s.degree_v(),"control_point_count_u":s.control_point_count_u(),"control_point_count_v":s.control_point_count_v(),
        "knots_u":s.knots_u(),"knots_v":s.knots_v(),"control_points":s.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>()})
}

#[test]
fn planar_image_protocol_distinguishes_zero_geometric_error_from_uv_correspondence() {
    use viboceros_oracle::{ProbeRequest, run_request};
    let create = |x: &[f64]| {
        NurbsSurface::try_new(
            x.len() - 1,
            1,
            x.len(),
            2,
            [0., 1.]
                .into_iter()
                .flat_map(|y| x.iter().map(move |&x| Point3::try_new(x, y, 0.).unwrap()))
                .collect(),
            [vec![0.; x.len()], vec![1.; x.len()]].concat(),
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    };
    let a = create(&[0., 1.]);
    let b = create(&[0., 1.1, -0.1, 1.]);
    let mut operation = json!({"op":"surface_planar_image_bound","id":"image","first":definition(&a),"second":definition(&b),"absolute_tolerance":0.});
    let call = |operation: &Value| {
        run_request(&serde_json::from_value::<ProbeRequest>(json!({"protocol_version":1,"iterations":1,"tolerance":{"absolute":0.2,"relative":1e-9,"angular":1e-6},"operations":[operation]})).unwrap())
    };
    let output = call(&operation).unwrap();
    assert_eq!(
        output.results[0].value,
        json!({"certified":true,"maximum_deviation_bound":0.})
    );
    assert!(
        a.parameter_surface_deviation_bound(&b, 1e-9)
            .unwrap()
            .is_none()
    );
    let shifted = a
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(0., 0., 0.125).unwrap(),
        ))
        .unwrap();
    operation["second"] = definition(&shifted);
    operation
        .as_object_mut()
        .unwrap()
        .remove("absolute_tolerance");
    let bound = call(&operation).unwrap().results[0].value["maximum_deviation_bound"]
        .as_f64()
        .unwrap();
    assert!((0.125..=0.2).contains(&bound));
    operation["absolute_tolerance"] = json!(0.125);
    assert_eq!(
        call(&operation).unwrap().results[0].value["maximum_deviation_bound"],
        0.125
    );
    operation["absolute_tolerance"] = json!(0.1);
    assert_eq!(
        call(&operation).unwrap().results[0].value["certified"],
        false
    );
    operation["absolute_tolerance"] = json!(-1.);
    assert!(call(&operation).is_err());
}
