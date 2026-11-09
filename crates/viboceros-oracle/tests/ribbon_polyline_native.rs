//! Ordinary polyline Ribbon support tensors and native topology, at 1e-9.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
#[path = "../../viboceros-command/tests/support/interpolation_plane.rs"]
#[allow(dead_code)]
mod curves;
#[path = "support/ribbon.rs"]
#[allow(dead_code)]
mod support;

#[test]
fn ordinary_polyline_ribbons_compare_complete_faces_and_expose_remaining_gaps() {
    let q: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/rhino_oracle/fixtures/ribbon_polyline.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let n = curves::read("ribbon_polyline");
    let mut matched = 0;
    let mut incidences = 0;
    assert_eq!(q["operations"].as_array().unwrap().len(), 52);
    assert_eq!(n["results"].as_array().unwrap().len(), 52);
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(n["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let v = &row["value"];
        assert_eq!(v["success"], true);
        let curve = curves::curve(&v["before"][0]["curve"]);
        assert_eq!(curve.degree(), 1);
        let source = Polyline3::try_with_parameters(
            curve.control_points().iter().map(|p| p.point()).collect(),
            curve.knots()[1..curve.knots().len() - 1].to_vec(),
            captured::model_tolerance(),
        )
        .unwrap();
        let normal = Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let sign = if op["both_sides"] == true {
            1.
        } else {
            Curve3::Polyline(source.clone())
                .offset_side(
                    captured::point(&op["side"]),
                    normal,
                    captured::model_tolerance(),
                )
                .unwrap()
        };
        let corner = match op["corner"].as_str().unwrap() {
            "Sharp" => CurveOffsetCornerStyle::Sharp,
            "Round" => CurveOffsetCornerStyle::Round,
            "Chamfer" => CurveOffsetCornerStyle::Chamfer,
            "None" => CurveOffsetCornerStyle::None,
            _ => panic!("unqualified corner"),
        };
        let before = source.clone();
        let d = sign * op["distance"].as_f64().unwrap();
        #[cfg(feature = "native-smlib")]
        let parts = try_polyline_ribbon_with_ruled(
            &source,
            d,
            op["both_sides"] == true,
            normal,
            captured::model_tolerance(),
            corner,
            &mut viboceros_smlib::ruled_surface,
        )
        .unwrap();
        #[cfg(not(feature = "native-smlib"))]
        let parts = try_polyline_ribbon(
            &source,
            d,
            op["both_sides"] == true,
            normal,
            captured::model_tolerance(),
            corner,
        )
        .unwrap();
        assert_eq!(source, before);
        let brep = Brep::try_combine(parts, captured::model_tolerance()).unwrap();
        let output = v["created"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["source"] == false)
            .unwrap();
        let targets = output["surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .map(captured::surface)
            .collect::<Vec<_>>();
        let mut used = vec![false; targets.len()];
        let mut matches = Vec::new();
        for f in brep.faces() {
            let m = targets
                .iter()
                .enumerate()
                .filter(|(i, _)| !used[*i])
                .find_map(|(i, s)| {
                    f.surface()
                        .parameter_surface_deviation_bound(s, 1e-9)
                        .unwrap()
                        .map(|b| (i, b))
                });
            if let Some((i, b)) = m {
                used[i] = true;
                matches.push(json!({"native_index":i,"bound":b}));
            } else {
                matches.push(Value::Null);
            }
        }
        let all = matches.len() == targets.len() && matches.iter().all(|m| !m.is_null());
        let coverage = if all {
            support::coverage(
                &brep,
                &brep
                    .faces()
                    .iter()
                    .map(|f| f.surface().clone())
                    .collect::<Vec<_>>(),
                &output["brep"]["geometry"],
                &targets,
            )
        } else {
            None
        };
        let incidence = if all {
            support::incidence(
                &brep,
                &output["brep"]["geometry"],
                &matches
                    .iter()
                    .map(|m| m["native_index"].as_u64().unwrap() as usize)
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        if all {
            matched += 1;
        }
        let gap = op["corner"] == "Sharp" && op["id"].as_str().unwrap().starts_with("zigzag");
        assert_eq!(all, !gap, "{}: full face matching", op["id"]);
        if !gap {
            assert!(
                coverage.is_some_and(|b| b <= 1e-9),
                "{}: complete image",
                op["id"]
            );
        } else {
            assert!(coverage.is_none());
        }
        let point_gap = op["corner"] == "None" && op["both_sides"] == false;
        if gap || point_gap {
            assert!(incidence.is_none(), "{}: retained incidence gap", op["id"]);
        } else {
            assert!(
                incidence.is_some_and(|b| b <= 1e-9),
                "{}: incidence",
                op["id"]
            );
            incidences += 1;
        }
        let saved = captured::archive(&brep);
        assert_eq!(
            [
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            ],
            [
                saved.faces().len(),
                saved.edges().len(),
                saved.vertices().len()
            ]
        );
        let mut archive_bound = 0_f64;
        for (a, b) in brep.faces().iter().zip(saved.faces()) {
            archive_bound = archive_bound.max(
                a.surface()
                    .parameter_surface_deviation_bound(b.surface(), 1e-9)
                    .unwrap()
                    .unwrap(),
            );
        }
        let orientation = all.then(|| {
            brep.faces().iter().zip(&matches).all(|(f, m)| {
                f.is_reversed()
                    == output["brep"]["geometry"]["topology"]["faces"]
                        [m["native_index"].as_u64().unwrap() as usize]["reversed"]
                        .as_bool()
                        .unwrap()
            })
        });
        println!(
            "RIBBON_POLYLINE {}",
            json!({"id":op["id"],"counts":[brep.faces().len(),brep.edges().len(),brep.vertices().len()],
            "native_faces":targets.len(),"matches":matches,"support_coverage_bound":coverage,"incidence_bound":incidence,
            "archive_bound":archive_bound,"native_face_orientation_equal":orientation,"full_ribbon_command":false})
        );
    }
    assert_eq!(matched, 50);
    assert_eq!(incidences, 42);
}

#[test]
fn python_geometry_operation_keeps_charts_and_returns_components_without_command_claims() {
    use viboceros_oracle::{ProbeRequest, run_request};
    let mut op = json!({"op":"polyline_ribbon_geometry","id":"ribbon",
        "vertices":[[0.,0.,0.],[5.,0.,0.],[5.,4.,0.]],"parameters":[-4.,0.,8.],
        "distance":-0.5,"both_sides":true,"corner":"Round"});
    let call = |op: &Value| {
        run_request(
            &serde_json::from_value::<ProbeRequest>(
                json!({"protocol_version":1,"iterations":1,"operations":[op]}),
            )
            .unwrap(),
        )
    };
    let v = call(&op).unwrap().results.remove(0).value;
    assert_eq!(v["before"], v["after"]);
    assert_eq!(v["before"]["parameters"], op["parameters"]);
    assert_eq!(v["full_ribbon_command"], false);
    assert_eq!(v["distance_certified"], false);
    assert_eq!(v["breps"].as_array().unwrap().len(), 1);
    assert_eq!(v["breps"][0]["faces"].as_array().unwrap().len(), 7);
    op["both_sides"] = json!(false);
    op["corner"] = json!("None");
    assert_eq!(
        call(&op).unwrap().results[0].value["breps"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    op["corner"] = json!("Smooth");
    assert!(call(&op).is_err());
    op["corner"] = json!("Sharp");
    op["distance"] = json!(0.);
    assert!(call(&op).is_err());
    op["distance"] = json!(0.5);
    op["parameters"] = json!([0., 1.]);
    assert!(call(&op).is_err());
}
