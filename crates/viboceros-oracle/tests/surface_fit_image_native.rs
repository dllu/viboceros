//! Image fitting and native image agreement are separate from UV correspondence.
use serde_json::{Value, json};
use viboceros_geometry::*;
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;

fn fit(
    source: &NurbsSurface,
    options: SurfaceFitOptions,
    limit: f64,
) -> Option<BoundedSurfaceImageFit> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::fit_surface_image_with_bound(source, options, limit).unwrap()
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        try_fit_surface_image_with_bound(source, options, limit).unwrap()
    }
}

#[test]
fn image_fitting_covers_all_planar_native_outputs_and_preserves_curved_declines() {
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    let mut cases = 0;
    let mut planar = 0;
    for family in ["surface_fit", "surface_fit_adaptive", "surface_fit_strips"] {
        let query: Value = serde_json::from_slice(
            &std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../../tools/rhino_oracle/fixtures/{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let native = captured::read(family);
        let operations = query["operations"].as_array().unwrap();
        let rows = native["results"].as_array().unwrap();
        assert_eq!(operations.len(), rows.len());
        for (op, row) in operations.iter().zip(rows) {
            assert_eq!(op["id"], row["id"]);
            let value = &row["value"];
            assert_eq!(value["before"], value["after"]);
            let source = captured::surface(&value["before"]);
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
            let target =
                (!value["surface"].is_null()).then(|| captured::surface(&value["surface"]));
            let flat = source.control_points().iter().all(|p| p.point().z() == 0.);
            let mut parameter_bound = None;
            let mut planar_bound = None;
            let mut archive_bound = None;
            if let Some(fit) = &actual {
                let s = &fit.surface;
                assert_eq!([s.degree_u(), s.degree_v()], options.degree);
                assert!(s.control_point_count_u() <= 16 && s.control_point_count_v() <= 16);
                assert_eq!(s.domain_u(), source.domain_u());
                assert_eq!(s.domain_v(), source.domain_v());
                assert!((0. ..=limit).contains(&fit.maximum_deviation_bound));
                let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
                    s.clone(),
                    s.domain_u(),
                    s.domain_v(),
                    false,
                    captured::model_tolerance(),
                )
                .unwrap();
                let saved = captured::archive(&brep);
                archive_bound = s
                    .parameter_surface_deviation_bound(saved.faces()[0].surface(), 1e-9)
                    .unwrap();
                assert!(archive_bound.is_some(), "{}: archive", op["id"]);
                if let Some(target) = &target {
                    parameter_bound = s.parameter_surface_deviation_bound(target, 1e-9).unwrap();
                    planar_bound = s
                        .planar_surface_image_deviation_bound(target, 1e-9)
                        .unwrap();
                }
            }
            if flat {
                assert!(target.is_some());
                assert!(
                    planar_bound.is_some(),
                    "{}: complete native planar image",
                    op["id"]
                );
                let s = &actual.as_ref().unwrap().surface;
                assert_eq!(
                    [s.control_point_count_u(), s.control_point_count_v()],
                    options.degree.map(|d| d + 1)
                );
                // Independently recheck every planar source's complete image.
                assert!(
                    source
                        .planar_surface_image_deviation_bound(s, limit)
                        .unwrap()
                        .is_some()
                );
                planar += 1;
            }
            let status = match (&actual, &target) {
                (Some(_), Some(_)) if parameter_bound.or(planar_bound).is_some() => {
                    "native_image_match"
                }
                (Some(_), Some(_)) => "native_image_unproved",
                (Some(_), None) => "native_null_with_fit",
                (None, Some(_)) => "native_surface_without_fit",
                (None, None) => "both_declined",
            };
            *counts.entry(status).or_default() += 1;
            cases += 1;
            println!(
                "SURFACE_IMAGE_FIT {}",
                json!({"id":op["id"],"status":status,"planar_fixture":flat,
                "source_bound":actual.as_ref().map(|f|f.maximum_deviation_bound),"native_parameter_bound":parameter_bound,
                "native_planar_bound":planar_bound,"archive_bound":archive_bound,"limit":limit,"maximum_controls":[16,16],
                "controls":actual.as_ref().map(|f|[f.surface.control_point_count_u(),f.surface.control_point_count_v()])})
            );
        }
    }
    assert_eq!((cases, planar), (112, 30));
    println!(
        "SURFACE_IMAGE_FIT_TOTAL {}",
        json!({"cases":cases,"counts":counts,"planar_native_matches":planar,"full_rhino_fit_parity":false})
    );
}

fn call(op: &Value) -> Result<Value, viboceros_oracle::ProbeError> {
    let query = serde_json::from_value::<ProbeRequest>(json!({"protocol_version":1,"iterations":1,
        "tolerance":{"absolute":0.01,"relative":1e-9,"angular":1e-6},"operations":[op]}))
    .unwrap();
    Ok(run_request(&query)?.results.remove(0).value)
}

fn planar_fixture() -> Value {
    let q: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/rhino_oracle/fixtures/surface_fit_strips.json"),
        )
        .unwrap(),
    )
    .unwrap();
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"] == "fit-rational-plane-0-1")
        .unwrap()["surface"]
        .clone()
}

#[test]
fn json_image_metric_defaults_and_zero_are_distinct_from_parameter_fitting() {
    let mut op = json!({"op":"surface_fit_image_bounded","id":"image","surface":planar_fixture(),"degree_u":1,"degree_v":1,"maximum_control_count":[2,2],"absolute_tolerance":0.});
    let value = call(&op).unwrap();
    assert!(!value["surface"].is_null());
    assert_eq!(value["maximum_deviation_bound"], 0.);
    assert_eq!(value["before"], value["after"]);
    let a = captured::surface(&value["before"]);
    let b = captured::surface(&value["surface"]);
    assert!(
        a.parameter_surface_deviation_bound(&b, 0.01)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        a.planar_surface_image_deviation_bound(&b, 0.).unwrap(),
        Some(0.)
    );
    op["op"] = json!("surface_fit_bounded");
    assert!(call(&op).unwrap()["surface"].is_null());
    op["op"] = json!("surface_fit_image_bounded");
    op.as_object_mut().unwrap().remove("maximum_control_count");
    op.as_object_mut().unwrap().remove("absolute_tolerance");
    let defaults = call(&op).unwrap();
    op["absolute_tolerance"] = Value::Null;
    assert_eq!(call(&op).unwrap(), defaults);
    op["absolute_tolerance"] = json!(0.01);
    op["maximum_control_count"] = json!([64, 64]);
    assert_eq!(call(&op).unwrap(), defaults);
    for (key, value) in [
        ("degree_u", json!(0)),
        ("degree_v", json!(12)),
        ("maximum_control_count", json!([1, 2])),
        ("absolute_tolerance", json!(-1.)),
    ] {
        let mut invalid = op.clone();
        invalid[key] = value;
        assert!(call(&invalid).is_err());
    }
}
