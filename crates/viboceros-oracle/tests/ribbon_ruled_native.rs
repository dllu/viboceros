//! Independent loose Ribbon tensors and shells; native differences stay visible.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
#[path = "../../viboceros-command/tests/support/interpolation_plane.rs"]
#[allow(dead_code)]
mod curves;
#[path = "ribbon_ruled_native/support.rs"]
mod support;

fn build(
    s: &NurbsCurve,
    d: f64,
    both: bool,
    n: UnitVector3,
    t: Tolerance,
) -> Result<(Brep, Vec<NurbsSurface>), String> {
    let mut parents = Vec::new();
    #[cfg(feature = "native-smlib")]
    {
        let brep = try_loose_ribbon_with_ruled(
            s,
            d,
            both,
            n,
            t,
            Some(viboceros_smlib::curve_edit_point_interpolation),
            &mut |a, b| {
                let surface = viboceros_smlib::ruled_surface(a, b)?;
                parents.push(surface.try_reparameterized(0. ..=d.abs(), s.domain())?);
                Ok::<_, viboceros_smlib::Error>(surface)
            },
        )
        .map_err(|e| e.to_string())?;
        Ok((brep, parents))
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let brep = try_loose_ribbon_with_ruled(s, d, both, n, t, None, &mut |a, b| {
            let surface = NurbsSurface::try_ruled_from_curves(a, b)?;
            parents.push(surface.try_reparameterized(0. ..=d.abs(), s.domain())?);
            Ok::<_, GeometryError>(surface)
        })
        .map_err(|e| e.to_string())?;
        Ok((brep, parents))
    }
}

#[test]
fn loose_ribbon_shells_certify_complete_native_images_and_retain_the_incidence_gap() {
    let q: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/rhino_oracle/fixtures/ribbon_ruled.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let n = curves::read("ribbon_ruled");
    let mut cases = 0;
    let mut incidence_matches = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(n["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let v = &row["value"];
        assert_eq!(v["success"], true);
        let source = curves::curve(&v["before"][0]["curve"]);
        let before = source.clone();
        let t = Tolerance::try_new(1e-6, 1e-9, 1e-6).unwrap();
        let normal = Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let side =
            Point3::try_from(serde_json::from_value::<[f64; 3]>(op["side"].clone()).unwrap())
                .unwrap();
        let both = op["both_sides"].as_bool().unwrap();
        let sign = if both {
            1.
        } else {
            Curve3::NurbsCurve(source.clone())
                .offset_side(side, normal, t)
                .unwrap()
        };
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
        let result = build(
            &source,
            sign * op["distance"].as_f64().unwrap(),
            both,
            normal,
            t,
        );
        let mut matches = Vec::new();
        let mut counts = None;
        let mut error = None;
        let mut archive_bound = None;
        let mut coverage = None;
        let mut incidence = None;
        let mut orientation = None;
        match result {
            Err(e) => error = Some(e),
            Ok((brep, parents)) => {
                coverage =
                    support::coverage(&brep, &parents, &output["brep"]["geometry"], &targets);
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
                let mut archive = 0_f64;
                for (a, b) in brep.faces().iter().zip(saved.faces()) {
                    archive = archive.max(
                        a.surface()
                            .parameter_surface_deviation_bound(b.surface(), 1e-9)
                            .unwrap()
                            .unwrap(),
                    );
                }
                archive_bound = Some(archive);
                counts = Some([
                    brep.faces().len(),
                    brep.edges().len(),
                    brep.vertices().len(),
                ]);
                let mut used = vec![false; targets.len()];
                for face in brep.faces() {
                    let found = targets
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !used[*i])
                        .find_map(|(i, s)| {
                            face.surface()
                                .parameter_surface_deviation_bound(s, 1e-9)
                                .unwrap()
                                .map(|b| (i, b))
                        });
                    if let Some((i, b)) = found {
                        used[i] = true;
                        matches.push(json!({"native_index":i,"bound":b}));
                    } else {
                        matches.push(Value::Null);
                    }
                }
                if matches.len() == targets.len() && matches.iter().all(|m| !m.is_null()) {
                    let mapping = matches
                        .iter()
                        .map(|m| m["native_index"].as_u64().unwrap() as usize)
                        .collect::<Vec<_>>();
                    incidence = support::incidence(&brep, &output["brep"]["geometry"], &mapping);
                    orientation = Some(brep.faces().iter().zip(&mapping).all(|(f, &i)| {
                        f.is_reversed()
                            == output["brep"]["geometry"]["topology"]["faces"][i]["reversed"]
                                .as_bool()
                                .unwrap()
                    }));
                }
            }
        }
        assert_eq!(source, before);
        assert!(error.is_none(), "{}: {error:?}", op["id"]);
        assert!(
            coverage.is_some_and(|b| b <= 1e-9),
            "{}: complete image",
            op["id"]
        );
        if op["id"] == "ribbon-native-polycurve-both" {
            assert_eq!(counts.unwrap(), [4, 12, 9]);
            assert_eq!(targets.len(), 3);
            assert!(incidence.is_none());
        } else {
            assert!(
                incidence.is_some_and(|b| b <= 1e-9),
                "{}: incidence",
                op["id"]
            );
            incidence_matches += 1;
        }
        println!(
            "RIBBON_RULED {}",
            json!({"id":op["id"],"counts":counts,"native_faces":targets.len(),
            "matches":matches,"error":error,"archive_bound":archive_bound,"support_coverage_bound":coverage,
            "incidence_bound":incidence,"native_face_orientation_equal":orientation,"full_ribbon_command":false})
        );
        cases += 1;
    }
    assert_eq!(cases, 32);
    assert_eq!(incidence_matches, 31);
}

#[test]
fn json_loose_ribbon_returns_sewn_sides_without_a_command_or_distance_certificate() {
    use viboceros_oracle::{ProbeRequest, run_request};
    let mut op = json!({"op":"loose_ribbon_geometry","id":"ribbon","distance":0.5,"both_sides":true,
        "curve":{"degree":1,"knots":[0.,0.,1.,1.],"control_points":[
            {"point":[0.,0.,0.],"weight":1.},{"point":[5.,0.,0.],"weight":1.}]}});
    let call = |op: &Value| {
        run_request(
            &serde_json::from_value::<ProbeRequest>(
                json!({"protocol_version":1,"iterations":1,"operations":[op]}),
            )
            .unwrap(),
        )
    };
    let value = call(&op).unwrap().results.remove(0).value;
    assert_eq!(value["before"], value["after"]);
    assert_eq!(value["distance_certified"], false);
    assert_eq!(value["full_ribbon_command"], false);
    assert_eq!(value["brep"]["faces"].as_array().unwrap().len(), 2);
    assert_eq!(value["brep"]["edges"].as_array().unwrap().len(), 7);
    assert_eq!(value["brep"]["vertices"].as_array().unwrap().len(), 6);
    op["distance"] = json!(0.);
    assert!(call(&op).is_err());
}
