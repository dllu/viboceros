//! Complete ordinary smooth Ribbon tensors, topology and native gauges.
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

fn family(family: &str) -> usize {
    let mut cases = 0;
    let n = curves::read(family);
    let q: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../tools/rhino_oracle/fixtures/{family}.json")),
        )
        .unwrap(),
    )
    .unwrap();
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
        if family == "ribbon_options"
            && ![
                "line",
                "line-both",
                "line-input-layer",
                "circle-outside",
                "circle-inside",
                "circle-both",
                "smooth",
            ]
            .contains(&op["id"].as_str().unwrap())
        {
            continue;
        }
        assert_eq!(op["id"], row["id"]);
        let v = &row["value"];
        assert_eq!(v["success"], true);
        let source = curves::curve(&v["before"][0]["curve"]);
        let original = source.clone();
        let normal = Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let t = captured::model_tolerance();
        let sign = if op["both_sides"] == true {
            1.
        } else {
            Curve3::NurbsCurve(source.clone())
                .offset_side(captured::point(&op["side"]), normal, t)
                .unwrap()
        };
        let d = sign * op["distance"].as_f64().unwrap();
        let mut parents = Vec::new();
        let started = std::time::Instant::now();
        #[cfg(feature = "native-smlib")]
        let result = try_smooth_ribbon_with_ruled(
            &source,
            d,
            op["both_sides"] == true,
            normal,
            t,
            &mut |a, b| {
                let surface = viboceros_smlib::ruled_surface(a, b)?;
                parents.push(surface.try_reparameterized(0. ..=d.abs(), source.domain())?);
                Ok::<_, viboceros_smlib::Error>(surface)
            },
        )
        .map_err(|e| e.to_string());
        #[cfg(not(feature = "native-smlib"))]
        let result = try_smooth_ribbon_with_ruled(
            &source,
            d,
            op["both_sides"] == true,
            normal,
            t,
            &mut |a, b| {
                let surface = NurbsSurface::try_ruled_from_curves(a, b)?;
                parents.push(surface.try_reparameterized(0. ..=d.abs(), source.domain())?);
                Ok::<_, GeometryError>(surface)
            },
        )
        .map_err(|e| e.to_string());
        let construction_ns = started.elapsed().as_nanos();
        assert_eq!(source, original);
        let verifying = std::time::Instant::now();
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
        let cache = std::cell::RefCell::new(Vec::<(NurbsSurface, NurbsSurface, f64)>::new());
        let compare = |a: &NurbsSurface, b: &NurbsSurface, limit: f64| {
            if let Some((_, _, bound)) = cache
                .borrow()
                .iter()
                .find(|(x, y, bound)| x == a && y == b && *bound <= limit)
            {
                return Some(*bound);
            }
            let witness = source.try_trimmed(a.domain_v()).ok()?;
            let bound = a
                .ruled_surface_image_deviation_bound_with_witnesses(
                    b,
                    [Some(&witness), None],
                    limit,
                )
                .ok()
                .flatten()?;
            cache.borrow_mut().push((a.clone(), b.clone(), bound));
            Some(bound)
        };
        let mut matches = Vec::new();
        let mut coverage = None;
        let mut incidence = None;
        let mut counts = None;
        let mut archive_bound = None;
        let mut error = None;
        match result {
            Err(e) => error = Some(e),
            Ok(brep) => {
                let witnesses = brep
                    .faces()
                    .iter()
                    .flat_map(|f| {
                        let w = source.try_trimmed(f.surface().domain_v()).unwrap();
                        [w.clone(), w.reversed().unwrap()]
                    })
                    .collect::<Vec<_>>();
                let compare_edge = |a: &NurbsCurve, b: &NurbsCurve, limit: f64| {
                    for w in &witnesses {
                        if [a, b].into_iter().any(|c| {
                            [false, true].into_iter().any(|end| {
                                let p = |c: &NurbsCurve| {
                                    c.evaluate(if end {
                                        *c.domain().end()
                                    } else {
                                        *c.domain().start()
                                    })
                                    .unwrap()
                                };
                                p(c).distance_to(p(w)).unwrap() > limit
                            })
                        }) {
                            continue;
                        }
                        if let (Some(x), Some(y)) = (
                            a.parameter_curve_deviation_bound(w, limit * 0.5)
                                .ok()
                                .flatten(),
                            b.parameter_curve_deviation_bound(w, limit * 0.5)
                                .ok()
                                .flatten(),
                        ) {
                            let sum = x + y;
                            let bound = if sum == 0. { 0. } else { sum.next_up() };
                            if bound <= limit {
                                return Some(bound);
                            }
                        }
                    }
                    a.parameter_curve_deviation_bound(b, limit).ok().flatten()
                };
                coverage = support::coverage_with(
                    &brep,
                    &parents,
                    &output["brep"]["geometry"],
                    &targets,
                    &compare,
                );
                counts = Some([
                    brep.faces().len(),
                    brep.edges().len(),
                    brep.vertices().len(),
                ]);
                let mut used = vec![false; targets.len()];
                for f in brep.faces() {
                    let found = targets
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !used[*i])
                        .find_map(|(i, s)| compare(f.surface(), s, 1e-9).map(|b| (i, b)));
                    if let Some((i, b)) = found {
                        used[i] = true;
                        matches.push(json!({"native_index":i,"bound":b}));
                    } else {
                        matches.push(Value::Null);
                    }
                }
                if matches.len() == targets.len() && matches.iter().all(|m| !m.is_null()) {
                    incidence = support::incidence_with(
                        &brep,
                        &output["brep"]["geometry"],
                        &matches
                            .iter()
                            .map(|m| m["native_index"].as_u64().unwrap() as usize)
                            .collect::<Vec<_>>(),
                        &compare_edge,
                    );
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
                let mut maximum = 0_f64;
                for (a, b) in brep.faces().iter().zip(saved.faces()) {
                    maximum = maximum.max(
                        a.surface()
                            .parameter_surface_deviation_bound(b.surface(), 1e-9)
                            .unwrap()
                            .unwrap(),
                    );
                }
                archive_bound = Some(maximum);
            }
        }
        let verification_ns = verifying.elapsed().as_nanos();
        println!(
            "RIBBON_SMOOTH {}",
            json!({"family":family,"id":op["id"],"counts":counts,"native_faces":targets.len(),
            "matches":matches,"support_coverage_bound":coverage,"incidence_bound":incidence,"archive_bound":archive_bound,"error":error,
            "full_ribbon_command":false,"distance_certified":false,"construction_ns":construction_ns,"verification_ns":verification_ns})
        );
        assert!(error.is_none(), "{family}/{}: {error:?}", op["id"]);
        assert!(
            coverage.is_some_and(|b| b <= 1e-9),
            "{family}/{}: complete image",
            op["id"]
        );
        assert!(
            incidence.is_some_and(|b| b <= 1e-9),
            "{family}/{}: incidence",
            op["id"]
        );
        assert_eq!(matches.len(), targets.len());
        assert!(matches.iter().all(|m| !m.is_null()));
        cases += 1;
    }
    cases
}
#[test]
fn ordinary_smooth_ribbons_compare_complete_native_images() {
    for (name, count) in [
        ("ribbon_options", 7),
        ("ribbon_smooth", 37),
        ("ribbon_analytic", 8),
        ("ribbon_near_analytic", 8),
    ] {
        assert_eq!(family(name), count);
    }
}

#[test]
fn smooth_and_ruled_image_json_operations_keep_geometry_and_certificate_scopes_explicit() {
    use viboceros_oracle::{ProbeRequest, run_request};
    let mut op = json!({"op":"smooth_ribbon_geometry","id":"smooth","distance":0.2,
        "curve":{"degree":1,"knots":[-4.,-4.,8.,8.],"control_points":[
            {"point":[0.,0.,0.],"weight":1.},{"point":[5.,0.,0.],"weight":1.}]}});
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
    assert_eq!(v["distance_certified"], false);
    assert_eq!(v["full_ribbon_command"], false);
    let s = &v["brep"]["faces"][0]["definition"];
    let first = json!({"degree_u":s["degree"][0],"degree_v":s["degree"][1],
        "control_point_count_u":s["control_count"][0],"control_point_count_v":s["control_count"][1],
        "control_points":s["control_points"],"knots_u":s["knots_u"],"knots_v":s["knots_v"],
        "domain_u":s["domain_u"],"domain_v":s["domain_v"]});
    let mut second = first.clone();
    second["control_points"][0]["weight"] = json!(3.);
    second["control_points"][2]["weight"] = json!(3.);
    let image = json!({"op":"surface_ruled_image_bound","id":"image","first":first,"second":second,"absolute_tolerance":0.});
    let bound = call(&image).unwrap().results.remove(0).value;
    assert_eq!(bound["certified"], true);
    assert_eq!(bound["maximum_deviation_bound"], 0.);
    assert_eq!(bound["u_correspondence_certified"], false);
    op["distance"] = json!(0.);
    assert!(call(&op).is_err());
}
