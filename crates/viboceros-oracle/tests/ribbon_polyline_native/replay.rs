//! Complete surface, archive and incidence replay shared by the holdout families.
use super::*;
pub(super) fn family(family: &str, expected: usize) {
    let q: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../tools/rhino_oracle/fixtures/{family}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    let n = curves::read(family);
    let mut matched = 0;
    let mut incidences = 0;
    assert_eq!(
        q["operations"].as_array().unwrap().len(),
        n["results"].as_array().unwrap().len()
    );
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(n["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        if op["op"] != "ribbon_command" {
            continue;
        }
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
            "Sharp" => RibbonCornerStyle::Sharp,
            "Round" => RibbonCornerStyle::Round,
            "Smooth" => RibbonCornerStyle::Smooth,
            "Chamfer" => RibbonCornerStyle::Chamfer,
            "None" => RibbonCornerStyle::None,
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
        assert!(
            all,
            "{family}/{}: full face matching: {matches:?}",
            op["id"]
        );
        assert!(
            coverage.is_some_and(|b| b <= 1e-9),
            "{family}/{}: complete image",
            op["id"]
        );
        {
            assert!(
                incidence.is_some_and(|b| b <= 1e-9),
                "{family}/{}: incidence; local {:?}, native {:?}; local edge vertices {:?}",
                op["id"],
                [
                    brep.faces().len(),
                    brep.edges().len(),
                    brep.vertices().len()
                ],
                [
                    targets.len(),
                    output["brep"]["geometry"]["edges"]
                        .as_array()
                        .unwrap()
                        .len(),
                    output["brep"]["geometry"]["vertices"]
                        .as_array()
                        .unwrap()
                        .len()
                ],
                brep.edges()
                    .iter()
                    .map(|e| e.vertices().map(|i| brep.vertices()[i].point().to_array()))
                    .collect::<Vec<_>>()
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
            "RIBBON_CORNERS {}",
            json!({"family":family,"id":op["id"],"counts":[brep.faces().len(),brep.edges().len(),brep.vertices().len()],
            "native_faces":targets.len(),"matches":matches,"support_coverage_bound":coverage,"incidence_bound":incidence,
            "archive_bound":archive_bound,"native_face_orientation_equal":orientation,"full_ribbon_command":false})
        );
    }
    assert_eq!(matched, expected);
    assert_eq!(incidences, expected);
}
