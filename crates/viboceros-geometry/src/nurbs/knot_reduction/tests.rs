use super::*;
use crate::Point3;

fn polynomial(points: Vec<[Real; 3]>, degree: usize, knots: Vec<Real>) -> NurbsCurve {
    NurbsCurve::try_new(
        degree,
        points
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        knots,
    )
    .unwrap()
}

fn corner() -> NurbsCurve {
    polynomial(
        vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
            [3., 0., 0.],
            [3., 1., 0.],
            [3., 2., 0.],
            [3., 3., 0.],
        ],
        3,
        [vec![0.; 4], vec![0.5; 3], vec![1.; 4]].concat(),
    )
}

#[test]
fn zero_is_strict_and_unrestricted_removal_is_explicit() {
    let source = corner();
    let before = source.clone();
    let strict = source.try_reduce_knots_in_range(1, 4, Some(0.)).unwrap();
    assert_eq!(strict.curve, source);
    assert_eq!(strict.removed, 0);
    assert_eq!(strict.maximum_deviation_bound, Some(0.));
    let loose = source.try_reduce_knots_in_range(1, 4, None).unwrap();
    assert_eq!(loose.removed, 2);
    assert_eq!(loose.curve.control_points().len(), 5);
    assert_eq!(loose.maximum_deviation_bound, None);
    assert!(
        source
            .parameter_curve_deviation_bound(&loose.curve, 1e-3)
            .unwrap()
            .is_none()
    );
    assert_eq!(source, before);
}

#[test]
fn constant_curves_reduce_exactly_under_both_weight_gauges() {
    for gauge in [1., -2.] {
        let source = NurbsCurve::try_new_rational(
            3,
            vec![
                WeightedPoint3::try_new(Point3::try_from([2., 3., 4.]).unwrap(), gauge).unwrap();
                6
            ],
            [vec![0.; 4], vec![0.5; 2], vec![1.; 4]].concat(),
        )
        .unwrap();
        let before = source.clone();
        let fit = source
            .try_reduce_knot_multiplicity_with_bound(0.5, 0, 0.)
            .unwrap()
            .unwrap();
        assert_eq!(fit.removed, 2);
        assert_eq!(fit.curve.control_points().len(), 4);
        assert_eq!(fit.maximum_deviation_bound, Some(0.));
        assert_eq!(source, before);
    }
}

#[test]
fn a_fitted_cubic_join_can_reduce_to_c1_without_forcing_c2() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/knot_reduction.json");
    let query: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let op = query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| {
            op["id"]
                .as_str()
                .unwrap()
                .starts_with("knot-reduction-fresh-half-join-")
        })
        .unwrap();
    let d = &op["curve"];
    let controls = d["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|cp| {
            Point3::try_from(serde_json::from_value::<[Real; 3]>(cp["point"].clone()).unwrap())
                .unwrap()
        })
        .collect();
    let source = NurbsCurve::try_new(
        3,
        controls,
        serde_json::from_value(d["knots"].clone()).unwrap(),
    )
    .unwrap();
    let before = source.clone();
    let reduced = source
        .try_reduce_knot_multiplicity_with_bound(0.5, 2, 1e-6)
        .unwrap()
        .unwrap();
    assert_eq!(reduced.removed, 1);
    assert_eq!(reduced.curve.control_points().len(), 6);
    assert!(reduced.maximum_deviation_bound.unwrap() <= 1e-6);
    assert!(
        source
            .parameter_curve_deviation_bound(&reduced.curve, 1e-6)
            .unwrap()
            .is_some()
    );
    assert!(
        source
            .try_reduce_knot_multiplicity_with_bound(0.5, 1, 1e-6)
            .unwrap()
            .is_none()
    );
    assert_eq!(source, before);
}

#[test]
fn each_group_checks_the_original_curve_so_displacements_cannot_accumulate() {
    let source = polynomial(
        [0., 1., 2., 5., 7., 10., 11., 12.]
            .map(|x| [x, x, 0.])
            .to_vec(),
        3,
        [vec![0.; 4], vec![0.25; 2], vec![0.75; 2], vec![1.; 4]].concat(),
    );
    let before = source.clone();
    let mut calls = 0;
    let reduced = source
        .try_reduce_knots_in_range_with_remover(1, 4, Some(0.001), &mut |base,
                                                                         knots|
         -> Result<
            NurbsCurve,
            GeometryError,
        > {
            calls += 1;
            let proposal = portable_removal(base, knots)?;
            let controls = proposal
                .control_points
                .iter()
                .map(|cp| {
                    let mut p = cp.point().to_array();
                    p[1] += 0.0006;
                    WeightedPoint3::try_new(Point3::try_from(p)?, cp.weight())
                })
                .collect::<Result<Vec<_>, _>>()?;
            NurbsCurve::try_new_rational(proposal.degree, controls, proposal.knots)
        })
        .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(reduced.removed, 1);
    assert!(reduced.maximum_deviation_bound.unwrap() <= 0.001);
    assert!(
        source
            .parameter_curve_deviation_bound(&reduced.curve, 0.001)
            .unwrap()
            .is_some()
    );
    assert_eq!(source, before);
}

#[test]
fn a_callback_cannot_claim_to_have_removed_a_knot_in_the_wrong_basis() {
    let source = corner();
    let before = source.clone();
    let result = source.try_reduce_knots_in_range_with_remover(1, 4, None, &mut |base, _| {
        Ok::<_, GeometryError>(base.clone())
    });
    assert_eq!(result.unwrap_err(), GeometryError::KnotRemovalSolveFailed);
    assert_eq!(source, before);
}

#[test]
fn invalid_limits_ranges_and_excess_work_fail_without_editing() {
    let source = corner();
    let before = source.clone();
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert_eq!(
            source
                .try_reduce_knots_in_range(1, 4, Some(limit))
                .unwrap_err(),
            GeometryError::InvalidTolerance
        );
    }
    assert!(matches!(
        source.try_reduce_knots_in_range(2, 2, None),
        Err(GeometryError::InvalidKnotReductionRange { .. })
    ));
    assert_eq!(source, before);
    let count = MAX_KNOT_REDUCTION_GROUPS + 3;
    let points = (0..count).map(|i| [i as Real, 0., 0.]).collect();
    let knots = [
        vec![0., 0.],
        (1..count - 1).map(|i| i as Real).collect(),
        vec![(count - 1) as Real; 2],
    ]
    .concat();
    let source = polynomial(points, 1, knots);
    let mut called = false;
    assert_eq!(
        source
            .try_reduce_knots_in_range_with_remover(1, 2, None, &mut |base, _| {
                called = true;
                Ok::<_, GeometryError>(base.clone())
            })
            .unwrap_err(),
        GeometryError::CurveApproximationLimit
    );
    assert!(!called);
}

#[test]
fn unclamped_inputs_keep_their_domain_and_complete_original_curve_bound() {
    let source = polynomial(
        vec![
            [0., 0., 0.],
            [1., 1., 0.],
            [2., 2., 0.],
            [3., 1., 0.],
            [4., 0., 0.],
            [5., 3., 0.],
        ],
        2,
        vec![-2., -1., 0., 0.5, 0.5, 1., 2., 3., 4.],
    );
    let before = source.clone();
    let reduced = source.try_reduce_knots_in_range(1, 3, Some(10.)).unwrap();
    assert_eq!(reduced.removed, 1);
    assert_eq!(reduced.curve.domain(), source.domain());
    assert!(reduced.maximum_deviation_bound.unwrap() <= 10.);
    assert!(
        source
            .parameter_curve_deviation_bound(&reduced.curve, 10.)
            .unwrap()
            .is_some()
    );
    assert_eq!(source, before);
}

#[test]
fn unsupported_certificate_inputs_fail_before_calling_the_basis_editor() {
    let periodic = polynomial(
        vec![
            [0., 0., 0.],
            [3., -1., 0.],
            [5., 4., 0.],
            [0., 0., 0.],
            [3., -1., 0.],
        ],
        2,
        vec![-2., -1., 0., 1., 2., 3., 4., 5.],
    );
    let high = polynomial(
        vec![[0., 0., 0.]; 18],
        17,
        [vec![0.; 18], vec![1.; 18]].concat(),
    );
    let mixed = NurbsCurve::try_new_rational(
        3,
        [1., 1., -1., 1.]
            .map(|w| WeightedPoint3::try_new(Point3::try_from([0.; 3]).unwrap(), w).unwrap())
            .to_vec(),
        [vec![0.; 4], vec![1.; 4]].concat(),
    )
    .unwrap();
    for source in [periodic, high, mixed] {
        let before = source.clone();
        let mut called = false;
        let result =
            source.try_reduce_knots_in_range_with_remover(1, 4, Some(1.), &mut |base, _| {
                called = true;
                Ok::<_, GeometryError>(base.clone())
            });
        assert!(matches!(
            result,
            Err(GeometryError::PeriodicKnotRemovalUnsupported { .. }
                | GeometryError::UnsupportedCurveApproximation { .. })
        ));
        assert!(!called);
        assert_eq!(source, before);
    }
}
