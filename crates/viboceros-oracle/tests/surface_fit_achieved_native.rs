//! Public SDK tolerance reports and diagnostic witnesses remain distinct.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;

fn data() -> (Value, Value) {
    let q = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/rhino_oracle/fixtures/surface_fit_achieved.json"),
        )
        .unwrap(),
    )
    .unwrap();
    (q, captured::read("surface_fit_achieved"))
}
fn interpolate(a: &NurbsSurface, b: &NurbsSurface) -> NurbsSurface {
    let degree = [b.degree_u(), b.degree_v()];
    let knots = [b.knots_u(), b.knots_v()];
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::interpolate_surface_in_basis(a, degree, knots).unwrap()
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        try_interpolate_surface_in_basis(a, degree, knots).unwrap()
    }
}
fn bound(a: &NurbsSurface, b: &NurbsSurface) -> f64 {
    a.parameter_surface_deviation_bound(b, 1e-9)
        .unwrap()
        .unwrap()
}

#[test]
fn public_cpp_fits_match_managed_and_independent_controls_with_reported_tolerances() {
    let (q, n) = data();
    let operations = q["operations"].as_array().unwrap();
    let rows = n["results"].as_array().unwrap();
    assert_eq!(operations.len(), 15);
    assert_eq!(rows.len(), 15);
    let mut successes = 0;
    let mut nulls = 0;
    for (op, row) in operations.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        let v = &row["value"];
        assert_eq!(v["before"], v["after"]);
        assert_eq!(v["surface"], v["managed_surface"]);
        assert_eq!(v["module"], "RhinoCore.dll");
        assert_eq!(v["achieved_finite"], true);
        let source = captured::surface(&v["before"]);
        let before = source.clone();
        let authored = bound(&source, &captured::surface(&op["surface"]));
        let achieved = v["achieved_tolerance"].as_f64().unwrap();
        let limit = op["fit_tolerance"].as_f64().unwrap();
        assert!((0. ..=limit).contains(&achieved));
        let mut native_bound = None;
        let mut archive_bound = None;
        let mut source_parameter = None;
        let mut metrics = None;
        let mut counts = None;
        if v["surface"].is_null() {
            nulls += 1;
            assert_eq!(achieved, 0.);
        } else {
            successes += 1;
            let target = captured::surface(&v["surface"]);
            let actual = interpolate(&source, &target);
            assert_eq!(
                [target.degree_u(), target.degree_v()],
                [
                    op["degree_u"].as_u64().unwrap() as usize,
                    op["degree_v"].as_u64().unwrap() as usize
                ]
            );
            counts = Some([
                target.control_point_count_u(),
                target.control_point_count_v(),
            ]);
            native_bound = Some(bound(&actual, &target));
            let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
                actual.clone(),
                actual.domain_u(),
                actual.domain_v(),
                false,
                captured::model_tolerance(),
            )
            .unwrap();
            archive_bound = Some(bound(
                &actual,
                captured::archive(&brep).faces()[0].surface(),
            ));
            source_parameter = source
                .parameter_surface_deviation_bound(&target, limit)
                .unwrap();
            let sample = source
                .deviation_samples(
                    &target,
                    [16, 16],
                    Tolerance::try_new(1e-12, 1e-12, 1e-9).unwrap(),
                )
                .unwrap();
            assert_eq!(sample.first_to_second.refinement_failures, 0);
            assert_eq!(sample.second_to_first.refinement_failures, 0);
            metrics = Some([
                sample.same_parameters.maximum.distance,
                sample.first_to_second.maximum.distance,
                sample.second_to_first.maximum.distance,
            ]);
        }
        assert_eq!(source, before);
        println!(
            "FIT_ACHIEVED {}",
            json!({"id":op["id"],"requested_tolerance":limit,"reported_achieved":achieved,
            "authored_bound":authored,"native_bound":native_bound,"archive_bound":archive_bound,"source_parameter_bound":source_parameter,
            "counts":counts,"samples":metrics,"sampled_certified":false})
        );
    }
    assert_eq!((successes, nulls), (14, 1));
    println!(
        "FIT_ACHIEVED_TOTAL {}",
        json!({"observations":15,"captured_bases":14,"native_null":1,"adaptive_fit_parity":false})
    );
}

#[test]
fn identical_outputs_have_different_reports_and_a_naive_sample_cutoff_selects_another_basis() {
    let (_, n) = data();
    let rows = n["results"].as_array().unwrap();
    let value = |id: &str| rows.iter().find(|r| r["id"] == id).unwrap()["value"].clone();
    let a = value("achieved-strip-sweep-0.03");
    let b = value("achieved-strip-sweep-0.05");
    assert_eq!(a["before"], b["before"]);
    assert_eq!(a["surface"], b["surface"]);
    assert_ne!(a["achieved_tolerance"], b["achieved_tolerance"]);
    let a = value("achieved-fit-strip-rational_bezier-0-3-0.01");
    let b = value("achieved-fit-strip-rational_bezier-0-3-0.001");
    assert_eq!(a["before"], b["before"]);
    assert_eq!(a["surface"], b["surface"]);
    assert_ne!(a["achieved_tolerance"], b["achieved_tolerance"]);
    let source = captured::surface(&a["before"]);
    let target = captured::surface(&a["surface"]);
    let knots = |p: usize, d: std::ops::RangeInclusive<f64>| {
        [vec![*d.start(); p + 1], vec![*d.end(); p + 1]].concat()
    };
    let initial = try_interpolate_surface_in_basis(
        &source,
        [3, 1],
        [&knots(3, source.domain_u()), &knots(1, source.domain_v())],
    )
    .unwrap();
    let sample = source
        .deviation_samples(
            &initial,
            [16, 16],
            Tolerance::try_new(1e-12, 1e-12, 1e-9).unwrap(),
        )
        .unwrap();
    assert!(
        sample.first_to_second.maximum.distance < 0.01
            && sample.second_to_first.maximum.distance < 0.01
    );
    assert_eq!(initial.control_point_count_u(), 4);
    assert_eq!(target.control_point_count_u(), 7);
}

#[test]
fn sampled_json_protocol_reports_witnesses_without_an_accuracy_certificate() {
    use viboceros_oracle::{ProbeRequest, run_request};
    let surface = |z: f64| {
        json!({"degree_u":1,"degree_v":1,"control_point_count_u":2,"control_point_count_v":2,
        "knots_u":[0.,0.,1.,1.],"knots_v":[0.,0.,1.,1.],"control_points":[
        {"point":[0.,0.,z],"weight":1.},{"point":[1.,0.,z],"weight":1.},{"point":[0.,1.,z],"weight":1.},{"point":[1.,1.,z],"weight":1.}]})
    };
    let mut op = json!({"op":"surface_deviation_samples","id":"samples","first":surface(0.),"second":surface(0.125)});
    let call = |o: &Value| {
        run_request(
            &serde_json::from_value::<ProbeRequest>(
                json!({"protocol_version":1,"iterations":1,"operations":[o]}),
            )
            .unwrap(),
        )
    };
    let value = call(&op).unwrap().results.remove(0).value;
    assert_eq!(value["certified"], false);
    assert_eq!(value["first_to_second"]["sample_count"], 289);
    assert_eq!(value["same_parameters"]["maximum_witness_distance"], 0.125);
    assert_eq!(value["first_to_second"]["maximum_witness_distance"], 0.125);
    op["divisions"] = json!([2, 3]);
    assert_eq!(
        call(&op).unwrap().results[0].value["second_to_first"]["sample_count"],
        12
    );
    op["divisions"] = json!([0, 3]);
    assert!(call(&op).is_err());
}
