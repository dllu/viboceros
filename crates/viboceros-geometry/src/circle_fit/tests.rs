use super::*;
use serde_json::Value;

fn inputs() -> (Value, Value) {
    (
        serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/fixtures/circle_fit_points.json"
        ))
        .unwrap(),
        serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/observations/circle_fit_points.json"
        ))
        .unwrap(),
    )
}
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}

#[test]
fn regular_native_fits_match_geometric_circles_and_keep_duplicate_weights() {
    let (fixture, observed) = inputs();
    for (op, row) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observed["results"].as_array().unwrap())
        .take(21)
    {
        let points = op["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect::<Vec<_>>();
        let fit = Circle3::try_fit_to_points(&points).unwrap();
        let c = &row["value"]["sdk"];
        if c["radius"] == 0. {
            assert!(fit.is_none());
            continue;
        }
        let fit = fit.unwrap();
        let center_error = fit.center().distance_to(point(&c["origin"])).unwrap();
        let radius_error = (fit.radius() - c["radius"].as_f64().unwrap()).abs();
        assert!(
            center_error + radius_error <= 1e-7,
            "{}: center {center_error} radius {radius_error}",
            op["id"]
        );
        let normal = fit.normal().unwrap().as_vector();
        let native_normal = Vector3::try_from(point(&c["normal"]).to_array()).unwrap();
        assert!((normal.dot(native_normal).unwrap().abs() - 1.).abs() < 1e-12);
        // Native seams/directions are measured separately, never assumed equal.
        for i in 0..64 {
            let theta = std::f64::consts::TAU * i as Real / 64.;
            let p = point(&c["origin"])
                .translated(
                    Vector3::try_from(std::array::from_fn(|j| {
                        c["radius"].as_f64().unwrap()
                            * (theta.cos() * c["x"][j].as_f64().unwrap()
                                + theta.sin() * c["y"][j].as_f64().unwrap())
                    }))
                    .unwrap(),
                )
                .unwrap();
            let v = fit.center().vector_to(p).unwrap();
            let height = v.dot(normal).unwrap();
            let radial = v.length().unwrap().hypot(0.).powi(2) - height * height;
            assert!(
                height.hypot(radial.max(0.).sqrt() - fit.radius()) < 1e-7,
                "{}",
                op["id"]
            );
        }
    }
}

#[test]
fn permutation_translation_and_scaling_preserve_the_fitted_locus() {
    let (q, _) = inputs();
    let points = q["operations"][15]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect::<Vec<_>>();
    let first = Circle3::try_fit_to_points(&points).unwrap().unwrap();
    for scale in [0.001, 1., 1000.] {
        let translation = [100., -50., 17.];
        let mut changed = points
            .iter()
            .map(|p| {
                Point3::try_from(std::array::from_fn(|i| {
                    p.to_array()[i] * scale + translation[i]
                }))
                .unwrap()
            })
            .collect::<Vec<_>>();
        changed.reverse();
        let fit = Circle3::try_fit_to_points(&changed).unwrap().unwrap();
        let expected = Point3::try_from(std::array::from_fn(|i| {
            first.center().to_array()[i] * scale + translation[i]
        }))
        .unwrap();
        assert!(fit.center().distance_to(expected).unwrap() < 1e-9 * scale.max(1.));
        assert!((fit.radius() - first.radius() * scale).abs() < 1e-9 * scale.max(1.));
    }
    assert!(Circle3::try_fit_to_points(&points[..2]).is_err());
}

#[test]
fn near_collinear_native_disagreement_remains_a_retained_diagnostic() {
    let (q, r) = inputs();
    let pts = q["operations"][21]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect::<Vec<_>>();
    let fit = Circle3::try_fit_to_points(&pts).unwrap().unwrap();
    let native = &r["results"][21]["value"]["sdk"];
    assert!(fit.center().distance_to(point(&native["origin"])).unwrap() > 1e7);
    assert!(fit.radius().is_finite() && fit.radius() > 0.);
}
