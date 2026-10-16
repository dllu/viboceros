use super::*;
use serde_json::Value;
fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}

#[test]
fn sphere_fit_sdk_corpus_retains_the_shallow_cap_reference_discrepancy() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/sphere_fit_points.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/sphere_fit_points_rhino_20261016.json"
    ))
    .unwrap();
    let mut matched = 0;
    let mut diagnostic = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        let points = op["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(p)
            .collect::<Vec<_>>();
        let reference = &row["value"]["sdk"];
        let fit = SphereFit::try_fit_to_points(&points).ok().flatten();
        assert_eq!(
            fit.is_some(),
            reference["created"].as_bool().unwrap(),
            "{}",
            op["id"]
        );
        if let Some(fit) = fit {
            let error = fit.center().distance_to(p(&reference["center"])).unwrap()
                + (fit.radius() - reference["radius"].as_f64().unwrap()).abs();
            println!(
                "{} sphere parameter/image bound diagnostic {error}",
                op["id"]
            );
            if op["id"] == "shallow-cap" {
                assert!(
                    fit.center()
                        .distance_to(Point3::try_new(0., 0., 0.).unwrap())
                        .unwrap()
                        + (fit.radius() - 2.).abs()
                        < 1e-6
                );
                assert!(error > 1.);
                for point in points {
                    assert!(
                        (point.distance_to(fit.center()).unwrap() - fit.radius()).abs() < 1e-10
                    );
                }
                diagnostic += 1;
            } else {
                assert!(error <= 1e-6, "{}: {error}", op["id"]);
                matched += 1;
            }
        }
    }
    assert_eq!((matched, diagnostic), (17, 1));
}

#[test]
fn sphere_fit_degenerate_and_resource_limits_do_not_create_invalid_parameters() {
    let p = Point3::try_new(1., 2., 3.).unwrap();
    assert!(SphereFit::try_fit_to_points(&[p; 2]).is_err());
    assert!(SphereFit::try_fit_to_points(&[p; 4]).unwrap().is_none());
    assert!(matches!(
        SphereFit::try_fit_to_points(&vec![p; MAX_SPHERE_FIT_POINTS + 1]),
        Err(GeometryError::SphereFitResourceLimit { .. })
    ));
}

#[test]
fn sphere_fit_tetrahedron_is_stable_under_binary_scale_and_source_permutations() {
    for scale in [2_f64.powi(-100), 1., 2_f64.powi(100)] {
        let mut points = [[1., 1., 1.], [1., -1., -1.], [-1., 1., -1.], [-1., -1., 1.]]
            .map(|a| Point3::try_from(a.map(|x| x * scale)).unwrap());
        let fit = SphereFit::try_fit_to_points(&points).unwrap().unwrap();
        assert!(
            fit.center()
                .distance_to(Point3::try_new(0., 0., 0.).unwrap())
                .unwrap()
                / scale
                < 1e-12
        );
        assert!((fit.radius() / scale - 3_f64.sqrt()).abs() < 1e-12);
        points.reverse();
        let other = SphereFit::try_fit_to_points(&points).unwrap().unwrap();
        assert!(
            fit.center().distance_to(other.center()).unwrap() / scale
                + (fit.radius() - other.radius()).abs() / scale
                < 1e-12
        );
    }
}
