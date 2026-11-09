//! Automatic bounded fitting and the distinct, representation-dependent SDK policy.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;

fn data(family: &str) -> (Value, Value) {
    let query = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../tools/rhino_oracle/fixtures/{family}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    (query, captured::read(family))
}
fn fit(source: &NurbsSurface, options: SurfaceFitOptions, limit: f64) -> Option<BoundedSurfaceFit> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::fit_surface_with_bound(source, options, limit).unwrap()
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        try_fit_surface_with_bound(source, options, limit).unwrap()
    }
}
fn interpolate(source: &NurbsSurface, expected: &NurbsSurface) -> NurbsSurface {
    let degree = [expected.degree_u(), expected.degree_v()];
    let knots = [expected.knots_u(), expected.knots_v()];
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::interpolate_surface_in_basis(source, degree, knots).unwrap()
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        try_interpolate_surface_in_basis(source, degree, knots).unwrap()
    }
}
fn archive_bound(surface: &NurbsSurface) -> f64 {
    let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
        surface.clone(),
        surface.domain_u(),
        surface.domain_v(),
        false,
        captured::model_tolerance(),
    )
    .unwrap();
    let archive = captured::archive(&brep);
    let bound = surface
        .parameter_surface_deviation_bound(archive.faces()[0].surface(), 1e-9)
        .unwrap();
    if bound.is_none() {
        let saved = archive.faces()[0].surface();
        eprintln!(
            "ARCHIVE_FIT_DIAGNOSTIC {}",
            json!({"equal":surface==saved,"degree":[surface.degree_u(),surface.degree_v()],"controls":[surface.control_point_count_u(),surface.control_point_count_v()],"knots_equal":surface.knots_u()==saved.knots_u() && surface.knots_v()==saved.knots_v(),"controls_equal":surface.control_points()==saved.control_points()})
        );
    }
    bound.unwrap()
}

#[test]
fn automatic_fits_certify_inputs_and_keep_native_policy_differences_explicit() {
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    let mut cases = 0;
    for family in ["surface_fit", "surface_fit_adaptive", "surface_fit_strips"] {
        let (query, native) = data(family);
        let operations = query["operations"].as_array().unwrap();
        let rows = native["results"].as_array().unwrap();
        assert_eq!(operations.len(), rows.len());
        for (op, row) in operations.iter().zip(rows) {
            assert_eq!(op["id"], row["id"]);
            assert_eq!(row["value"]["before"], row["value"]["after"]);
            let source = captured::surface(&row["value"]["before"]);
            let before = source.clone();
            let options = SurfaceFitOptions {
                degree: [
                    op["degree_u"].as_u64().unwrap() as usize,
                    op["degree_v"].as_u64().unwrap() as usize,
                ],
                maximum_control_count: [16, 16],
            };
            let limit = op["fit_tolerance"].as_f64().unwrap();
            let actual = fit(&source, options, limit);
            assert_eq!(source, before);
            let target = (!row["value"]["surface"].is_null())
                .then(|| captured::surface(&row["value"]["surface"]));
            let mut native_bound = None;
            let mut saved_bound = None;
            let mut source_bound = None;
            if let Some(actual) = &actual {
                let s = &actual.surface;
                assert_eq!([s.degree_u(), s.degree_v()], options.degree);
                assert!(s.control_point_count_u() <= 16 && s.control_point_count_v() <= 16);
                assert_eq!(s.domain_u(), source.domain_u());
                assert_eq!(s.domain_v(), source.domain_v());
                assert!(actual.maximum_deviation_bound <= limit);
                for j in 0..=8 {
                    for i in 0..=8 {
                        let u = source.parameter_at_u(i as f64 / 8.).unwrap();
                        let v = source.parameter_at_v(j as f64 / 8.).unwrap();
                        assert!(
                            source
                                .evaluate(u, v)
                                .unwrap()
                                .distance_to(s.evaluate(u, v).unwrap())
                                .unwrap()
                                <= actual.maximum_deviation_bound + 1e-12
                        );
                    }
                }
                source_bound = Some(actual.maximum_deviation_bound);
                saved_bound = Some(archive_bound(s));
                if let Some(target) = &target {
                    native_bound = s.parameter_surface_deviation_bound(target, 1e-9).unwrap();
                }
            }
            let status = match (&actual, &target) {
                (Some(_), Some(_)) if native_bound.is_some() => "native_match",
                (Some(_), Some(_)) => "native_correspondence_unproved",
                (Some(_), None) => "native_null_with_fit",
                (None, Some(_)) => "native_surface_without_fit",
                (None, None) => "both_declined",
            };
            *counts.entry(status).or_default() += 1;
            cases += 1;
            println!(
                "AUTOMATIC_SURFACE_FIT {}",
                json!({"id":op["id"],"status":status,"source_bound":source_bound,"native_bound":native_bound,"archive_bound":saved_bound,"limit":limit,"maximum_controls":[16,16],"controls":actual.as_ref().map(|r|[r.surface.control_point_count_u(),r.surface.control_point_count_v()])})
            );
        }
    }
    assert_eq!(cases, 112);
    println!(
        "AUTOMATIC_SURFACE_FIT_TOTAL {}",
        json!({"cases":cases,"counts":counts,"full_rhino_fit_parity":false})
    );
}

#[test]
fn new_captured_bases_reproduce_native_controls_independently_of_selection() {
    let mut matched = 0;
    let mut native_null = 0;
    for family in ["surface_fit_adaptive", "surface_fit_strips"] {
        let (query, native) = data(family);
        assert_eq!(
            query["operations"].as_array().unwrap().len(),
            native["results"].as_array().unwrap().len()
        );
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let value = &row["value"];
            assert_eq!(value["before"], value["after"]);
            if value["surface"].is_null() {
                native_null += 1;
                continue;
            }
            let source = captured::surface(&value["before"]);
            let expected = captured::surface(&value["surface"]);
            let actual = interpolate(&source, &expected);
            let bound = actual
                .parameter_surface_deviation_bound(&expected, 1e-9)
                .unwrap()
                .unwrap_or_else(|| panic!("{}: native basis controls", op["id"]));
            let archive = archive_bound(&actual);
            matched += 1;
            println!(
                "ADAPTIVE_SURFACE_BASIS {}",
                json!({"id":op["id"],"native_bound":bound,"archive_bound":archive})
            );
        }
    }
    assert_eq!((matched, native_null), (67, 9));
}

#[test]
fn exact_input_refinement_can_change_native_fitting_outcomes() {
    let (query, native) = data("surface_fit_adaptive");
    let operations = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    let mut pairs = 0;
    let mut changed = 0;
    let mut bounds = std::collections::HashMap::new();
    for (i, op) in operations.iter().enumerate() {
        let id = op["id"].as_str().unwrap();
        if !id.contains("-0-") || id.contains("small-plane") {
            continue;
        }
        let other = id.replacen("-0-", "-1-", 1);
        let j = operations.iter().position(|o| o["id"] == other).unwrap();
        let a = &rows[i]["value"];
        let b = &rows[j]["value"];
        let source_bound = *bounds
            .entry(json!([a["before"], b["before"]]).to_string())
            .or_insert_with(|| {
                captured::surface(&a["before"])
                    .parameter_surface_deviation_bound(&captured::surface(&b["before"]), 1e-9)
                    .unwrap()
                    .unwrap()
            });
        let signature = |v: &Value| {
            if v["surface"].is_null() {
                Value::Null
            } else {
                v["surface"]["control_count"].clone()
            }
        };
        let different = signature(a) != signature(b);
        changed += usize::from(different);
        pairs += 1;
        println!(
            "FIT_REPRESENTATION {}",
            json!({"id":id,"refined_id":other,"source_bound":source_bound,"native_inventory_changed":different,"original_controls":signature(a),"refined_controls":signature(b)})
        );
    }
    assert_eq!(pairs, 16);
    assert!(changed > 0);
}

#[test]
fn bounded_json_api_preserves_inputs_and_validates_limits() {
    use viboceros_oracle::{ProbeRequest, run_request};
    let (query, _) = data("surface_fit_strips");
    let mut op = json!({"op":"surface_fit_bounded","id":"bounded-fit","surface":query["operations"][0]["surface"],"degree_u":3,"degree_v":1,"maximum_control_count":[16,2],"absolute_tolerance":0.01});
    let call = |op: &Value| {
        run_request(
            &serde_json::from_value::<ProbeRequest>(
                json!({"protocol_version":1,"iterations":1,"operations":[op]}),
            )
            .unwrap(),
        )
    };
    let output = call(&op).unwrap();
    let value = &output.results[0].value;
    assert_eq!(value["before"], value["after"]);
    assert!(!value["surface"].is_null());
    assert!(value["maximum_deviation_bound"].as_f64().unwrap() <= 0.01);
    for change in [
        json!({"absolute_tolerance":-1.}),
        json!({"degree_u":0}),
        json!({"maximum_control_count":[3,2]}),
        json!({"maximum_control_count":[65,2]}),
    ] {
        let mut invalid = op.clone();
        for (k, v) in change.as_object().unwrap() {
            invalid[k] = v.clone();
        }
        assert!(call(&invalid).is_err());
    }
    op["maximum_control_count"] = json!([4, 2]);
    op["absolute_tolerance"] = json!(1e-12);
    let declined = call(&op).unwrap();
    assert!(declined.results[0].value["surface"].is_null());
    assert!(declined.results[0].value["maximum_deviation_bound"].is_null());
}
