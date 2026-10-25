use super::*;
use crate::ParameterSide;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tol(absolute: Real) -> Tolerance {
    Tolerance::try_new(absolute, 1e-12, 1e-10).unwrap()
}
fn quarter(scale: Real, gauge: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [
            (p(scale, 0., 0.), 1.),
            (p(scale, scale, 0.), std::f64::consts::FRAC_1_SQRT_2),
            (p(0., scale, 0.), 1.),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w * gauge).unwrap())
        .to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
fn checked(source: &NurbsCurve, error: Real) -> NurbsCurve {
    let original = source.clone();
    let (fit, bound) = source.try_approximate_cubic_with_bound(tol(error)).unwrap();
    assert_eq!(source, &original);
    assert_eq!(fit.degree(), 3);
    assert_eq!(source.domain(), fit.domain());
    assert!(bound.is_finite() && bound <= error);
    assert!(fit.control_points().len() <= MAX_CURVE_FIT_CONTROL_POINTS);
    assert!(fit.control_points().iter().all(|p| p.weight() == 1.));
    for i in 0..=256 {
        let t = source.parameter_at(i as Real / 256.).unwrap();
        let a = source.evaluate(t).unwrap();
        let b = fit.evaluate(t).unwrap();
        assert!(
            a.distance_to(b).unwrap() <= error * 1.001,
            "station {i}: {a:?} {b:?} {bound}"
        );
    }
    fit
}

#[test]
fn polynomial_cubics_and_constant_spans_keep_their_exact_controls() {
    let points = vec![p(0., 0., 0.), p(1., 3., 2.), p(2., -1., 1.), p(4., 0., 3.)];
    let original =
        NurbsCurve::try_new(3, points.clone(), vec![7., 7., 7., 7., 11., 11., 11., 11.]).unwrap();
    let fit = checked(&original, 1e-20);
    assert_eq!(fit, original);
    let constant = NurbsCurve::try_new(1, vec![p(2., 3., 4.); 2], vec![5., 5., 8., 8.]).unwrap();
    let (fit, bound) = constant
        .try_approximate_cubic_with_bound(tol(1e-20))
        .unwrap();
    assert_eq!(bound, 0.);
    assert_eq!(fit.control_points().len(), 4);
    assert!(
        fit.control_points()
            .iter()
            .all(|c| c.point() == p(2., 3., 4.))
    );
}

#[test]
fn rational_arcs_support_large_small_and_negative_weight_gauges() {
    for gauge in [1., -1., 1e-250, -1e250] {
        let source = quarter(2., gauge);
        let fit = checked(&source, 1e-6);
        assert!(fit.spans().count() > 1);
        assert!(
            fit.knots()
                .chunk_by(|a, b| a == b)
                .filter(|g| g[0] > 0. && g[0] < 1.)
                .all(|g| g.len() == 3)
        );
    }
}

#[test]
fn translated_and_tiny_coordinates_receive_a_stored_curve_certificate() {
    checked(&quarter(1e-280, 1.), 1e-285);
    let source = NurbsCurve::try_new_rational(
        2,
        quarter(1e190, 1.)
            .control_points()
            .iter()
            .map(|c| {
                let point = c.point().to_array();
                WeightedPoint3::try_new(
                    p(point[0] + 1e200, point[1] - 1e200, point[2] + 1e200),
                    c.weight(),
                )
                .unwrap()
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let (fit, bound) = source.try_approximate_cubic_with_bound(tol(1e185)).unwrap();
    assert!(bound <= 1e185);
    assert!(
        source
            .parameter_curve_deviation_bound(&fit, 1e185)
            .unwrap()
            .is_some()
    );
}

#[test]
fn kinks_and_genuine_one_sided_jumps_are_not_bridged() {
    for (points, knots, jump) in [
        (
            vec![p(0., 0., 0.), p(3., 0., 0.), p(3., 6., 0.)],
            vec![0., 0., 0.5, 1., 1.],
            false,
        ),
        (
            vec![p(0., 0., 0.), p(3., 0., 0.), p(5., 2., 0.), p(5., 8., 0.)],
            vec![0., 0., 0.5, 0.5, 1., 1.],
            true,
        ),
    ] {
        let source = NurbsCurve::try_new(1, points, knots).unwrap();
        let fit = checked(&source, 1e-20);
        assert_eq!(
            fit.knots().iter().filter(|&&k| k == 0.5).count(),
            if jump { 4 } else { 3 }
        );
        for side in [ParameterSide::Left, ParameterSide::Right] {
            assert_eq!(
                source.evaluate_on_side(0.5, side).unwrap(),
                fit.evaluate_on_side(0.5, side).unwrap()
            );
        }
    }
}

#[test]
fn unclamped_and_non_dyadic_native_domains_are_retained() {
    let source = NurbsCurve::try_new(
        2,
        vec![
            p(0., 0., 0.),
            p(2., 1., 0.),
            p(3., 0., 1.),
            p(4., 2., 0.),
            p(7., 3., 1.),
        ],
        vec![-2., -1., 0., 1., 2., 3., 4., 5.],
    )
    .unwrap();
    checked(&source, 1e-12);
    let source = quarter(1., 1.).try_reparameterized(0.1..=0.9).unwrap();
    checked(&source, 1e-8);
    let source = quarter(1., 1.)
        .try_reparameterized(1e200..=(1e200 + 1e190))
        .unwrap();
    checked(&source, 1e-6);
}

#[test]
fn source_loops_retain_their_closing_endpoint() {
    let source = crate::Circle3::try_new(
        p(0., 0., 0.),
        2.,
        crate::UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let fit = checked(&source, 1e-7);
    assert_eq!(
        fit.control_points().first().unwrap().point(),
        fit.control_points().last().unwrap().point()
    );
    assert!(crate::CurveRef::NurbsCurve(&fit).is_closed().unwrap());
}

#[test]
fn unsupported_degrees_weight_signs_and_unrepresentable_subdivision_fail() {
    let high = NurbsCurve::try_new(
        17,
        vec![p(0., 0., 0.); 18],
        [vec![0.; 18], vec![1.; 18]].concat(),
    )
    .unwrap();
    assert!(matches!(
        high.try_approximate_cubic(tol(1e-6)),
        Err(GeometryError::UnsupportedCurveApproximation { .. })
    ));
    let source = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(p(0., 0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(1., 1., 0.), -0.1).unwrap(),
            WeightedPoint3::try_new(p(2., 0., 0.), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert!(matches!(
        source.try_approximate_cubic(tol(1e-6)),
        Err(GeometryError::UnsupportedCurveApproximation { .. })
    ));
    let source = quarter(1., 1.)
        .try_reparameterized(1. ..=1_f64.next_up())
        .unwrap();
    assert_eq!(
        source.try_approximate_cubic(tol(1e-8)),
        Err(GeometryError::CurveApproximationLimit)
    );
}

#[test]
fn retained_source_spans_obey_the_output_control_limit() {
    let count = MAX_CURVE_FIT_CONTROL_POINTS / 3 + 2;
    let source = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 0.); count + 1],
        [
            vec![0.],
            (0..=count).map(|i| i as Real).collect(),
            vec![count as Real],
        ]
        .concat(),
    )
    .unwrap();
    assert_eq!(
        source.try_approximate_cubic(tol(1e-6)),
        Err(GeometryError::CurveApproximationLimit)
    );
}

#[test]
fn a_bump_hidden_between_uniform_samples_requires_refinement() {
    let source = NurbsCurve::try_new(
        5,
        [0., 6., -13., 13., -6., 0.]
            .into_iter()
            .enumerate()
            .map(|(i, y)| p(i as Real, y, 0.))
            .collect(),
        [vec![0.; 6], vec![1.; 6]].concat(),
    )
    .unwrap();
    for t in [0., 0.25, 0.5, 0.75, 1.] {
        assert_eq!(source.evaluate(t).unwrap().y(), 0.);
    }
    let fit = checked(&source, 1e-6);
    assert!(fit.spans().count() > 1);
    assert!((fit.evaluate(0.125).unwrap().y() - 1.025390625).abs() <= 1e-6);
}

#[test]
fn an_endpoint_that_cannot_be_rounded_within_tolerance_is_rejected() {
    assert!(matches!(
        quarter(1., 1.).try_approximate_cubic(tol(1e-30)),
        Err(GeometryError::UnsupportedCurveApproximation {
            context: "rounded endpoint exceeds the approximation tolerance"
        })
    ));
}

#[test]
fn overflowing_initial_handles_are_refined_in_exact_arithmetic() {
    let source = NurbsCurve::try_new(
        4,
        [Real::MAX, -Real::MAX, Real::MAX, -Real::MAX, Real::MAX]
            .map(|x| p(x, 0., 0.))
            .to_vec(),
        [vec![0.; 5], vec![1.; 5]].concat(),
    )
    .unwrap();
    let limit = Real::MAX / 100.;
    let (fit, bound) = source.try_approximate_cubic_with_bound(tol(limit)).unwrap();
    assert!(bound <= limit);
    assert!(fit.spans().count() > 1);
    assert!(
        fit.control_points()
            .iter()
            .all(|c| c.point().x().is_finite())
    );
}

#[test]
fn extreme_positive_weight_ratios_fail_within_the_subdivision_budget() {
    let source = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(p(0., 0., 0.), 1e-250).unwrap(),
            WeightedPoint3::try_new(p(1., 1., 0.), 1e250).unwrap(),
            WeightedPoint3::try_new(p(2., 0., 0.), 1e-250).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        source.parameter_curve_deviation_bound(&source, 0.).unwrap(),
        Some(0.)
    );
    assert_eq!(
        source.try_approximate_cubic(tol(1e-6)),
        Err(GeometryError::CurveApproximationLimit)
    );
}
