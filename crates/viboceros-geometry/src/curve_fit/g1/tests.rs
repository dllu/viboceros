use super::*;
use crate::{Point3, Real, WeightedPoint3};
use serde_json::Value;
use std::path::Path;

#[test]
fn image_agreement_does_not_erase_a_retraced_source_traversal() {
    let source = NurbsCurve::try_new(
        5,
        [0., 1., 1., 0., 0., 1.]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        [vec![0.; 6], vec![1.; 6]].concat(),
    )
    .unwrap();
    let candidate = initial_cubic(&source).unwrap();
    assert!(
        source
            .projected_curve_deviation_bound(&candidate, 1e-6)
            .unwrap()
            .is_some()
    );
    assert!(
        source
            .monotone_projected_curve_deviation_bound(&candidate, 1e-6)
            .unwrap()
            .is_none()
    );
    assert!(!within_deviation(&source, &candidate, 1e-6).unwrap());
}

#[test]
fn natural_knots_allow_complete_bounds_across_nonuniform_source_clocks() {
    for (start, width) in [(0., 1.), (-3., 10.)] {
        let source = NurbsCurve::try_new(
            1,
            [0., 0.125, 0.5, 1.]
                .map(|x| Point3::try_new(x, 0., 0.).unwrap())
                .to_vec(),
            // Unclamped end knots decline coordinate projection, retaining
            // coverage of the closest-point/natural-knot fallback.
            [-1., 0., 0.17, 0.63, 1., 2.]
                .map(|t| start + width * t)
                .to_vec(),
        )
        .unwrap();
        let before = source.clone();
        let candidate = initial_cubic(&source).unwrap();
        assert!(
            source
                .parameter_curve_deviation_bound(&candidate, 1e-10)
                .unwrap()
                .is_none()
        );
        assert!(within_deviation(&source, &candidate, 1e-10).unwrap());
        assert_eq!(source, before);
    }
}

#[test]
fn oversized_knot_sets_retain_the_uniform_correspondence_proposal() {
    let intervals = 257;
    let fractions = (0..=intervals)
        .map(|i| i as Real / intervals as Real)
        .collect::<Vec<_>>();
    let source = NurbsCurve::try_new(
        1,
        fractions
            .iter()
            .map(|t| Point3::try_new(t * t, 0., 0.).unwrap())
            .collect(),
        // Keep the projected proposal ineligible so this still exercises the
        // uniform fallback when natural boundaries exceed its interval budget.
        [vec![-1.], fractions, vec![2.]].concat(),
    )
    .unwrap();
    let candidate = initial_cubic(&source).unwrap();
    assert!(
        source
            .parameter_curve_deviation_bound(&candidate, 1e-5)
            .unwrap()
            .is_none()
    );
    assert!(within_deviation(&source, &candidate, 1e-5).unwrap());
}

pub(super) fn curve(d: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        d["degree"].as_u64().unwrap() as usize,
        d["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[Real; 3]>(c["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(d["knots"].clone()).unwrap(),
    )
    .unwrap()
}

#[test]
fn initial_endpoint_handles_match_all_ten_native_single_cubic_proposals() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_coarse.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let mut count = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        if op["refit_tolerance"] != 1. {
            continue;
        }
        let surface = &row["value"]["surfaces"][0];
        let counts = surface["control_count"].as_array().unwrap();
        if counts[1] != 4 {
            continue;
        }
        let nu = counts[0].as_u64().unwrap() as usize;
        let source = curve(&op["curves"][0]);
        let before = source.clone();
        let candidate = initial_cubic(&source).unwrap();
        for (i, point) in candidate.control_points().iter().enumerate() {
            let expected = Point3::try_from(
                serde_json::from_value::<[Real; 3]>(
                    surface["control_points"][i * nu]["point"].clone(),
                )
                .unwrap(),
            )
            .unwrap();
            let error = point.point().distance_to(expected).unwrap();
            assert!(error <= 1e-9, "{}: {error}", op["id"]);
            maximum = maximum.max(error);
        }
        assert_eq!(source, before);
        count += 1;
    }
    assert_eq!(count, 10);
    println!("G1_INITIAL records={count} maximum={maximum}");
}

#[test]
fn handle_geometry_survives_scale_domain_reversal_and_weight_gauges() {
    let points = [[0., 0., 0.], [1., 2., 1.], [3., -1., 0.], [4., 1., 0.]];
    let make = |scale: Real, gauge, shifted| {
        NurbsCurve::try_new_rational(
            3,
            points
                .map(|p| {
                    WeightedPoint3::try_new(Point3::try_from(p.map(|x| x * scale)).unwrap(), gauge)
                        .unwrap()
                })
                .to_vec(),
            if shifted {
                vec![-3.; 4].into_iter().chain(vec![7.; 4]).collect()
            } else {
                [vec![0.; 4], vec![1.; 4]].concat()
            },
        )
        .unwrap()
    };
    let reference = initial_cubic(&make(1., 1., false)).unwrap();
    for scale in [2_f64.powi(-500), 1., 2_f64.powi(500)] {
        for gauge in [2_f64.powi(-100), -1., 2_f64.powi(100)] {
            let source = make(scale, gauge, true);
            let before = source.clone();
            let candidate = initial_cubic(&source).unwrap();
            let reversed = initial_cubic(&source.reversed().unwrap()).unwrap();
            for ((actual, expected), reverse) in candidate
                .control_points()
                .iter()
                .zip(reference.control_points())
                .zip(reversed.control_points().iter().rev())
            {
                for ((a, e), r) in actual
                    .point()
                    .to_array()
                    .into_iter()
                    .zip(expected.point().to_array())
                    .zip(reverse.point().to_array())
                {
                    assert!((a / scale - e).abs() < 3e-15);
                    assert!((r / scale - e).abs() < 3e-15);
                }
                assert_eq!(actual.weight(), 1.);
            }
            assert_eq!(source, before);
        }
    }
}

#[test]
fn limiting_tangents_handle_stationary_ends_and_constant_curves_fail() {
    let source = NurbsCurve::try_new(
        3,
        [0., 0., 1., 1.]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        [vec![0.; 4], vec![1.; 4]].concat(),
    )
    .unwrap();
    let candidate = initial_cubic(&source).unwrap();
    for (i, c) in candidate.control_points().iter().enumerate() {
        assert!((c.point().x() - i as Real / 3.).abs() < 1e-15);
    }
    let constant = NurbsCurve::try_new(
        1,
        vec![Point3::try_new(0., 0., 0.).unwrap(); 2],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(initial_cubic(&constant).is_err());
}

#[test]
fn different_clocks_require_a_continuous_monotone_correspondence() {
    let source = NurbsCurve::try_new_rational(
        2,
        [
            ([2., 0., 0.], 1.),
            ([2., 2., 0.], 0.5_f64.sqrt()),
            ([0., 2., 0.], 1.),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
        .to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    let candidate = initial_cubic(&source).unwrap();
    assert!(
        source
            .parameter_curve_deviation_bound(&candidate, 0.165)
            .unwrap()
            .is_none()
    );
    assert!(within_deviation(&source, &candidate, 0.165).unwrap());
    assert!(!within_deviation(&source, &candidate, 0.16).unwrap());
    assert_eq!(source, before);
}
