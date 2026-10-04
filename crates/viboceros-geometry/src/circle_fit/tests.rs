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

fn assert_native_locus(fit: Circle3, native: &Value, epsilon: Real, label: &Value) {
    let normal = fit.normal().unwrap().as_vector();
    let native_normal = point(&native["normal"]).to_array();
    let normal_error = [-1., 1.]
        .into_iter()
        .map(|sign| {
            normal
                .to_array()
                .into_iter()
                .zip(native_normal)
                .fold(0.0_f64, |distance, (a, b)| distance.hypot(a - sign * b))
        })
        .fold(Real::INFINITY, Real::min);
    assert!(normal_error <= 1e-12, "{label}: normal {normal_error}");
    // Native seams/directions are measured separately, never assumed equal.
    // Boundary witnesses also catch normal errors amplified by a large radius.
    for i in 0..64 {
        let theta = std::f64::consts::TAU * i as Real / 64.;
        let p = point(&native["origin"])
            .translated(
                Vector3::try_from(std::array::from_fn(|j| {
                    native["radius"].as_f64().unwrap()
                        * (theta.cos() * native["x"][j].as_f64().unwrap()
                            + theta.sin() * native["y"][j].as_f64().unwrap())
                }))
                .unwrap(),
            )
            .unwrap();
        let v = fit.center().vector_to(p).unwrap();
        let height = v.dot(normal).unwrap();
        let radial = v.cross(normal).unwrap().length().unwrap();
        let distance = height.hypot(radial - fit.radius());
        assert!(
            distance <= epsilon,
            "{label}: boundary {distance} > {epsilon}"
        );
    }
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
        assert_native_locus(fit, c, 1e-7, &op["id"]);
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
fn near_collinear_native_circle_is_recovered_within_sixteen_radius_ulps() {
    let (q, r) = inputs();
    let pts = q["operations"][21]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect::<Vec<_>>();
    let fit = Circle3::try_fit_to_points(&pts).unwrap().unwrap();
    let native = &r["results"][21]["value"]["sdk"];
    let radius = native["radius"].as_f64().unwrap();
    let epsilon = 16. * (Real::from_bits(radius.to_bits() + 1) - radius);
    assert!(
        fit.center().distance_to(point(&native["origin"])).unwrap() + (fit.radius() - radius).abs()
            <= epsilon
    );
    assert!(fit.radius().is_finite() && fit.radius() > 0.);
    assert_native_locus(fit, native, epsilon, &q["operations"][21]["id"]);
}

#[test]
fn circle_fit_diagnostics_cover_center_witnesses_thin_arcs_and_large_circles() {
    for name in [
        "circle_fit_diagnostics",
        "circle_fit_distant_arcs",
        "circle_fit_distant_noisy",
    ] {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
        let q: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("fixtures").join(format!("{name}.json"))).unwrap(),
        )
        .unwrap();
        let r: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("observations").join(format!("{name}.json")))
                .unwrap(),
        )
        .unwrap();
        for (i, (op, row)) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(r["results"].as_array().unwrap())
            .enumerate()
        {
            let pts = op["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(point)
                .collect::<Vec<_>>();
            let native = &row["value"]["circle"];
            let radius = native["radius"].as_f64().unwrap();
            let fit = Circle3::try_fit_to_points(&pts).unwrap();
            if radius == 0. {
                assert!(fit.is_none());
                continue;
            }
            let fit = fit.unwrap();
            let error = fit.center().distance_to(point(&native["origin"])).unwrap()
                + (fit.radius() - radius).abs();
            if name == "circle_fit_diagnostics" && i == 24 {
                // Native's epsilon=1e-6 convergence remains measurably different.
                // Keep this gap explicit instead of widening the fit tolerance.
                assert!(error > 1e-7 && error < 1e-5);
                continue;
            }
            let epsilon = (16. * (Real::from_bits(radius.to_bits() + 1) - radius)).max(1e-7);
            assert!(
                error <= epsilon,
                "{}: error {error} epsilon {epsilon}",
                op["id"]
            );
            assert_native_locus(fit, native, epsilon, &op["id"]);
        }
    }
}
