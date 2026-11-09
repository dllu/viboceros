//! Controlled native strip observations; no adaptive native-policy claim.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;

fn data() -> (Value, Value) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/surface_fit_covariance.json");
    (
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap(),
        captured::read("surface_fit_covariance"),
    )
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
fn fit(a: &NurbsSurface, options: SurfaceFitOptions, limit: f64) -> BoundedSurfaceImageFit {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::fit_surface_image_with_bound(a, options, limit)
            .unwrap()
            .unwrap()
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        try_fit_surface_image_with_bound(a, options, limit)
            .unwrap()
            .unwrap()
    }
}
fn bound(a: &NurbsSurface, b: &NurbsSurface) -> f64 {
    a.parameter_surface_deviation_bound(b, 1e-9)
        .unwrap()
        .unwrap()
}
fn snapshot(s: &NurbsSurface) -> Value {
    json!({"degree":[s.degree_u(),s.degree_v()],
        "counts":[s.control_point_count_u(),s.control_point_count_v()],
        "knots":[s.knots_u(),s.knots_v()],
        "controls":s.control_points().iter().map(|p|(p.point().to_array(),p.weight())).collect::<Vec<_>>()})
}

#[test]
fn controlled_strips_reproduce_captured_bases_and_independently_certify_local_fits() {
    let (query, native) = data();
    let mut cases = 0;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        if op["op"] != "surface_fit_achieved" {
            continue;
        }
        let v = &row["value"];
        assert_eq!(v["before"], v["after"]);
        assert_eq!(v["surface"], v["managed_surface"]);
        assert_eq!(v["achieved_finite"], true);
        let limit = op["fit_tolerance"].as_f64().unwrap();
        assert!((0. ..=limit).contains(&v["achieved_tolerance"].as_f64().unwrap()));
        let source = captured::surface(&v["before"]);
        let original = source.clone();
        let target = captured::surface(&v["surface"]);
        let authored_bound = bound(&source, &captured::surface(&op["surface"]));
        let actual = interpolate(&source, &target);
        let native_bound = bound(&actual, &target);
        let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
            actual.clone(),
            actual.domain_u(),
            actual.domain_v(),
            false,
            captured::model_tolerance(),
        )
        .unwrap();
        let archive_bound = bound(&actual, captured::archive(&brep).faces()[0].surface());
        let options = SurfaceFitOptions {
            degree: [3, op["degree_v"].as_u64().unwrap() as usize],
            maximum_control_count: [16, op["degree_v"].as_u64().unwrap() as usize + 1],
        };
        let local = fit(&source, options, limit);
        assert_eq!(local.surface.domain_u(), source.domain_u());
        assert_eq!(local.surface.domain_v(), source.domain_v());
        let checked = local
            .certificate
            .verify(&source, &local.surface, limit)
            .unwrap();
        assert_eq!(checked, Some(local.maximum_deviation_bound));
        assert_eq!(source, original);
        println!(
            "FIT_COVARIANCE {}",
            json!({"id":op["id"],"limit":limit,
            "authored_bound":authored_bound,"native_bound":native_bound,"archive_bound":archive_bound,
            "local_bound":local.maximum_deviation_bound,"rechecked_bound":checked,
            "local_certificate":format!("{:?}",local.certificate),"local_surface":snapshot(&local.surface),
            "captured_basis":snapshot(&actual),"full_native_policy":false})
        );
        cases += 1;
    }
    assert_eq!(cases, 29);
}

fn normalize(s: &NurbsSurface, factors: [f64; 3]) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        s.degree_u(),
        s.degree_v(),
        s.control_point_count_u(),
        s.control_point_count_v(),
        s.control_points()
            .iter()
            .map(|p| {
                let q = p.point().to_array();
                WeightedPoint3::try_new(
                    Point3::try_from(std::array::from_fn(|i| q[i] * factors[i])).unwrap(),
                    p.weight(),
                )
                .unwrap()
            })
            .collect(),
        s.knots_u().to_vec(),
        s.knots_v().to_vec(),
    )
    .unwrap()
    .try_reparameterized(0. ..=1., 0. ..=1.)
    .unwrap()
}

#[test]
fn transverse_changes_preserve_native_curved_knots_but_curve_fit_uses_another_basis() {
    let (q, n) = data();
    let operations = q["operations"].as_array().unwrap();
    let rows = n["results"].as_array().unwrap();
    assert_eq!((operations.len(), rows.len()), (35, 35));
    let mut surfaces = 0;
    let mut curves = 0;
    for (op, row) in operations.iter().zip(rows) {
        let id = op["id"].as_str().unwrap();
        let v = &row["value"];
        if op["op"] == "curve_fit_geometry" {
            let native = &v["knots"];
            let a = v["domain"][0].as_f64().unwrap();
            let b = v["domain"][1].as_f64().unwrap();
            let knots = native
                .as_array()
                .unwrap()
                .iter()
                .map(|k| (k.as_f64().unwrap() - a) / (b - a))
                .collect::<Vec<_>>();
            // Unlike the captured surface fits, each curve has a non-dyadic knot.
            assert!(knots.iter().any(|t| (t * 4.).fract().abs() > 1e-6));
            println!(
                "FIT_COVARIANCE_CURVE {}",
                json!({"id":id,"knots":knots,
                "controls":v["control_points"].as_array().unwrap().len()})
            );
            curves += 1;
            continue;
        }
        let limit = op["fit_tolerance"].as_f64().unwrap();
        let mut factors = [1.; 3];
        let mut reference_limit = limit;
        if id.starts_with("width-") {
            factors[1] = 1. / id.split('-').nth(1).unwrap().parse::<f64>().unwrap();
        } else if id.starts_with("scale-") {
            factors = [1. / id.split('-').nth(1).unwrap().parse::<f64>().unwrap(); 3];
            reference_limit = id.split('-').nth(2).unwrap().parse().unwrap();
        }
        let reference_id = format!("width-1.0-{reference_limit}");
        let reference = rows.iter().find(|r| r["id"] == reference_id).unwrap();
        let a = normalize(&captured::surface(&v["surface"]), factors);
        let b = normalize(&captured::surface(&reference["value"]["surface"]), [1.; 3]);
        assert_eq!(a.knots_u(), b.knots_u());
        let covariance_bound = bound(&a, &b);
        println!(
            "FIT_COVARIANCE_NATIVE {}",
            json!({"id":id,"reference":reference_id,
            "bound":covariance_bound,"curved_controls":a.control_point_count_u()})
        );
        surfaces += 1;
    }
    assert_eq!((surfaces, curves), (29, 6));
}
