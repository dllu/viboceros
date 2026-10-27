use super::super::tests::curve;
use super::*;
use crate::Point3;
use serde_json::Value;
use std::path::Path;

#[test]
fn nearly_cancelling_handles_decline_an_ill_conditioned_correction() {
    let initial = NurbsCurve::try_new(
        3,
        [
            [0., 0., 0.],
            [2. / 3., 0., 0.],
            [4. / 3., 2e-5, 0.],
            [2., 2e-5, 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let before = initial.clone();
    assert!(
        adjusted_cubic_at_point(&initial, Point3::try_new(1., 5e-6, 0.).unwrap(), 1e-6)
            .unwrap()
            .is_none()
    );
    assert_eq!(initial, before);
}

fn read(family: &str, folder: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    serde_json::from_slice(&std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap())
        .unwrap()
}

#[test]
fn incremental_stations_match_public_nurbs_batches_at_both_precisions() {
    let mut count = 0;
    let mut maximum = 0_f64;
    for (family, precision) in [
        ("loft_refit_adjustment", 1e-6),
        ("loft_refit_adjustment_fit_clock", 1e-4),
        ("loft_refit_adjustment_ellipses", 1e-4),
    ] {
        let query = read(family, "fixtures");
        let native = read(family, "observations");
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let source = curve(&op["curves"][0]);
            let before = source.clone();
            let sampler =
                ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT)
                    .unwrap();
            let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
            let parameters = sampler
                .parameters_at_incremental_normalized_lengths(&fractions, precision)
                .unwrap();
            let reference = &row["value"]["refit_adjustment"];
            for (i, parameter) in parameters.into_iter().enumerate() {
                let error = (parameter - reference["parameters"][i].as_f64().unwrap()).abs();
                let point = Point3::try_from(
                    serde_json::from_value::<[Real; 3]>(reference["points"][i].clone()).unwrap(),
                )
                .unwrap();
                let distance = source
                    .evaluate(parameter)
                    .unwrap()
                    .distance_to(point)
                    .unwrap();
                assert!(
                    error <= 1e-9 && distance <= 1e-9,
                    "{} {i}: dt={error} dx={distance}",
                    op["id"]
                );
                maximum = maximum.max(error).max(distance);
                count += 1;
            }
            assert_eq!(source, before);
        }
    }
    assert_eq!(count, 891);
    println!("REFIT_ADJUST_CLOCK queries={count} maximum={maximum}");
}

#[test]
fn one_common_handle_correction_matches_all_native_adjusted_single_cubics() {
    let mut count = 0;
    let mut maximum = 0_f64;
    for family in [
        "loft_refit_adjustment_fit_clock",
        "loft_refit_adjustment_ellipses",
    ] {
        let query = read(family, "fixtures");
        let native = read(family, "observations");
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            let reference = &row["value"]["refit_adjustment"];
            let surface = &row["value"]["surfaces"][0];
            if surface["control_count"][1] != 4
                || reference["midpoint_distance"].as_f64().unwrap()
                    <= op["refit_tolerance"].as_f64().unwrap() * 0.5
            {
                continue;
            }
            let source = curve(&op["curves"][0]);
            let before = source.clone();
            let initial = super::super::initial_cubic(&source).unwrap();
            let saved = initial.clone();
            let adjusted = adjusted_cubic(
                &source,
                &initial,
                op["refit_tolerance"].as_f64().unwrap() * 0.5,
            )
            .unwrap()
            .unwrap();
            let nu = surface["control_count"][0].as_u64().unwrap() as usize;
            for (i, control) in adjusted.control_points().iter().enumerate() {
                let point = Point3::try_from(
                    serde_json::from_value::<[Real; 3]>(
                        surface["control_points"][i * nu]["point"].clone(),
                    )
                    .unwrap(),
                )
                .unwrap();
                let error = control.point().distance_to(point).unwrap();
                // The elliptical native correction differs slightly from the
                // public closest-point construction; Loft acceptance separately
                // certifies the complete candidate against its source.
                assert!(error <= 1e-8, "{} {i}: {error}", op["id"]);
                maximum = maximum.max(error);
            }
            assert_eq!(source, before);
            assert_eq!(initial, saved);
            count += 1;
        }
    }
    assert_eq!(count, 35);
    println!("REFIT_ADJUST_PROPOSAL records={count} maximum={maximum}");
}

#[test]
fn an_already_fitting_midpoint_cannot_hide_errors_elsewhere() {
    let source = NurbsCurve::try_new(
        3,
        [[0., 0., 0.], [1., 2., 0.], [3., -1., 0.], [4., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        [vec![0.; 4], vec![1.; 4]].concat(),
    )
    .unwrap();
    let initial = super::super::initial_cubic(&source).unwrap();
    let before = source.clone();
    let saved = initial.clone();
    assert!(adjusted_cubic(&source, &initial, 0.015).unwrap().is_none());
    assert!(!super::super::within_deviation(&source, &initial, 0.015).unwrap());
    assert_eq!(source, before);
    assert_eq!(initial, saved);
}
