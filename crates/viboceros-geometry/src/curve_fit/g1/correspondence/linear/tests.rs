use super::*;
use crate::{Point3, WeightedPoint3};

#[test]
fn composed_error_never_rounds_above_the_requested_limit() {
    let cost = 2_f64.powi(-54);
    assert_eq!(1. - cost, 1.);
    assert!(remaining_tolerance(1., cost).unwrap() < 1.);
    for limit in [1e-300_f64, 1e-6, 1., 1e200] {
        for error in [0., limit * 1e-12, limit * 0.125, limit.next_down()] {
            if let Some(remaining) = remaining_tolerance(limit, error) {
                use crate::exact_scalar::rational;
                assert!(rational(remaining) + rational(error) <= rational(limit));
            }
        }
        assert!(remaining_tolerance(limit, limit).is_none());
    }
}

#[test]
fn source_to_proxy_error_cannot_be_added_above_the_candidate_budget() {
    let limit = 1e-6;
    let source = NurbsCurve::try_new(
        3,
        [
            [0., 0., 0.],
            [1. / 3., 5e-13, 0.],
            [2. / 3., 5e-13, 0.],
            [1., 0., 0.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        [vec![0.; 4], vec![1.; 4]].concat(),
    )
    .unwrap();
    let proxy = source.try_change_degree(1, false).unwrap();
    let candidate = NurbsCurve::try_new(
        1,
        [[0., -limit, 0.], [1., -limit, 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(
        source
            .parameter_curve_deviation_bound(&proxy, limit * 1e-6)
            .unwrap()
            .is_some()
    );
    assert!(
        proxy
            .parameter_curve_deviation_bound(&candidate, limit)
            .unwrap()
            .is_some()
    );
    assert!(!certify(&source, &candidate, limit));
    assert!(!super::super::within_deviation(&source, &candidate, limit).unwrap());
}

#[test]
fn degree_gauge_chart_scale_and_reversal_preserve_complete_bounds() {
    for scale in [1e-100, 1., 1e100] {
        let polygon = NurbsCurve::try_new(
            1,
            [[0., 0., 0.], [1., 0., 0.], [2., 1e-5, 0.]]
                .map(|p| Point3::try_from(p.map(|v| v * scale)).unwrap())
                .to_vec(),
            vec![0., 0., 1., 2., 2.],
        )
        .unwrap();
        let candidate = polygon.try_change_degree(3, false).unwrap();
        let elevated = polygon.try_change_degree(5, false).unwrap();
        for gauge in [2_f64.powi(-100), -1., 2_f64.powi(100)] {
            let source = NurbsCurve::try_new_rational(
                5,
                elevated
                    .control_points()
                    .iter()
                    .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                    .collect(),
                elevated.knots().to_vec(),
            )
            .unwrap()
            .try_reparameterized(-3. ..=17.)
            .unwrap();
            let before = source.clone();
            assert!(certify(&source, &candidate, scale * 1e-6));
            assert!(certify(
                &source.reversed().unwrap(),
                &candidate.reversed().unwrap(),
                scale * 1e-6
            ));
            assert_eq!(source, before);
        }
    }
}

#[test]
fn degree_reduction_endpoints_cannot_hide_an_interior_feature() {
    let source = NurbsCurve::try_new(
        3,
        [
            [0., 0., 0.],
            [1. / 3., 0.01, 0.],
            [2. / 3., 0.01, 0.],
            [1., 0., 0.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        [vec![0.; 4], vec![1.; 4]].concat(),
    )
    .unwrap();
    let proxy = source.try_change_degree(1, false).unwrap();
    assert!(
        source
            .parameter_curve_deviation_bound(&proxy, 1e-12)
            .unwrap()
            .is_none()
    );
    assert!(!certify(&source, &proxy, 1e-6));
}

#[test]
#[ignore = "timing diagnostic for the saved elevated-curve certificates"]
fn saved_elevated_curve_timings() {
    use serde_json::Value;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let query: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/loft_refit_source_continuity.json")).unwrap(),
    )
    .unwrap();
    for i in [2, 3] {
        let op = &query["operations"][i];
        for definition in op["curves"].as_array().unwrap() {
            let source = crate::curve_fit::g1::tests::curve(definition);
            let before = source.clone();
            let start = std::time::Instant::now();
            let result = crate::try_propose_joined_refit(
                &source,
                op["refit_tolerance"].as_f64().unwrap(),
                0.,
            )
            .unwrap();
            assert!(result.is_some());
            println!(
                "CERTIFICATE_BENCH id={} z={} seconds={}",
                op["id"],
                source.control_points()[0].point().z(),
                start.elapsed().as_secs_f64()
            );
            assert_eq!(source, before);
        }
    }
}
