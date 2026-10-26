use super::*;
use crate::{NurbsCurve, WeightedPoint3};

fn rational_curve() -> NurbsCurve {
    NurbsCurve::try_new_rational(
        3,
        [[0., 0., 0.], [0.5, 2., 0.], [2., -1., 0.], [3., 0.5, 0.]]
            .into_iter()
            .zip([1., 1.4, 0.7, 1.2])
            .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn quadrature_queries_are_independent_of_order_repetitions_and_lookup_preparation() {
    let curve = rational_curve();
    let before = curve.clone();
    let mut sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let fractions = [0.875, 0.125, 0.5, 0.875, 1., 0., 0.125];
    let expected = fractions.map(|f| {
        sampler
            .parameter_at_quadrature_normalized_length(f, 1e-4)
            .unwrap()
    });
    assert_eq!(
        sampler
            .parameters_at_quadrature_normalized_lengths(&fractions, 1e-4)
            .unwrap(),
        expected
    );
    sampler.prepare_budgeted_repeated_sampling(32).unwrap();
    assert_eq!(
        sampler
            .parameters_at_quadrature_normalized_lengths(&fractions, 1e-4)
            .unwrap(),
        expected
    );
    assert_eq!(curve, before);
}

#[test]
fn independent_checkpoint_acceptance_retains_the_incremental_counterexample() {
    let curve = rational_curve();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let scalar = sampler
        .parameter_at_quadrature_normalized_length(0.125, 0.01)
        .unwrap();
    let batch = sampler
        .parameters_at_incremental_normalized_lengths(&[0.125], 0.01)
        .unwrap()[0];
    assert!(
        curve
            .evaluate(scalar)
            .unwrap()
            .distance_to(curve.evaluate(batch).unwrap())
            .unwrap()
            > 0.01
    );
}

#[test]
fn plateau_queries_choose_the_right_endpoint_and_keep_natural_domain_ends() {
    let curve = NurbsCurve::try_new(
        1,
        [[0., 0., 0.], [1., 0., 0.], [1., 0., 0.], [1., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![-3., -3., -1., 2., 7., 7.],
    )
    .unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    assert_eq!(
        sampler
            .parameters_at_quadrature_normalized_lengths(&[0., 0.5, 1.], 1e-4)
            .unwrap(),
        [-3., 2., 7.]
    );
}

#[test]
fn invalid_queries_do_not_mutate_or_poison_later_results() {
    let curve = rational_curve();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let before = sampler
        .parameter_at_quadrature_normalized_length(0.5, 1e-4)
        .unwrap();
    for f in [-1., 1.1, Real::NAN, Real::INFINITY] {
        assert!(
            sampler
                .parameter_at_quadrature_normalized_length(f, 1e-4)
                .is_err()
        );
    }
    for t in [-1., 0., 1., Real::NAN, Real::INFINITY] {
        assert_eq!(
            sampler.parameters_at_quadrature_normalized_lengths(&[], t),
            Err(GeometryError::InvalidArcLengthIntegrationTolerance)
        );
    }
    assert_eq!(
        sampler
            .parameter_at_quadrature_normalized_length(0.5, 1e-4)
            .unwrap(),
        before
    );
}

#[test]
fn numerical_queries_retain_nearby_knots_without_changing_precise_inverses() {
    let curve = NurbsCurve::try_new(
        1,
        [[0., 0., 0.], [1., 0., 0.], [1., 1.00001, 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 0.2, 1., 1.],
    )
    .unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let precise = sampler.parameters_at_normalized_lengths(&[0.5]).unwrap()[0];
    assert!(precise > 0.2);
    assert_eq!(
        sampler
            .parameter_at_quadrature_normalized_length(0.5, 1e-4)
            .unwrap(),
        0.2
    );
    assert!(
        (sampler
            .parameter_at_quadrature_normalized_length(0.5, 1e-8)
            .unwrap()
            - precise)
            .abs()
            < 1e-12
    );
    assert_eq!(
        sampler.parameters_at_normalized_lengths(&[0.5]).unwrap()[0],
        precise
    );
    let before = curve.clone();
    let point = curve.evaluate(0.2).unwrap();
    let midpoint = curve.evaluate(precise).unwrap();
    assert!(point.distance_to(midpoint).unwrap() <= 1e-4 * sampler.total_length());
    assert_eq!(curve, before);
}

#[test]
fn nearby_numerical_queries_choose_the_right_end_of_a_zero_length_plateau() {
    let curve = NurbsCurve::try_new(
        1,
        [[0., 0., 0.], [1., 0., 0.], [1., 0., 0.], [2.00001, 0., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![-3., -3., -1., 2., 7., 7.],
    )
    .unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    assert_eq!(
        sampler
            .parameters_at_quadrature_normalized_lengths(&[0.49999, 0.5, 0.50001], 1e-4)
            .unwrap(),
        [2., 2., 2.]
    );
}

#[test]
fn full_capacity_constant_speed_scalar_batches_preserve_checkpoint_policy() {
    let curve = NurbsCurve::try_new(
        1,
        [[0., 0., 0.], [1., 0., 0.], [1., 1.00001, 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let fractions = (0..MAX_LOOKUP_NODES)
        .map(|i| i as f64 / (MAX_LOOKUP_NODES - 1) as f64)
        .collect::<Vec<_>>();
    let values = sampler
        .parameters_at_quadrature_normalized_lengths(&fractions, 1e-8)
        .unwrap();
    assert_eq!(values.len(), fractions.len());
    assert_eq!(values[0], 0.);
    assert_eq!(*values.last().unwrap(), 2.);
    assert!(values.windows(2).all(|w| w[0] <= w[1]));
    assert_eq!(
        sampler
            .parameter_at_quadrature_normalized_length(0.5, 1e-4)
            .unwrap(),
        1.
    );
}

#[test]
fn analytic_polyline_clock_remains_exact_when_the_nurbs_clock_accepts_a_knot() {
    let points = [[0., 0., 0.], [1., 0., 0.], [1.98, 0.0000098, 0.]]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec();
    let curve = NurbsCurve::try_new(1, points.clone(), vec![0., 0., 1., 2., 2.]).unwrap();
    let polyline = crate::Polyline3::try_new(points, Tolerance::DEFAULT).unwrap();
    let nurbs =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let analytic =
        ArcLengthSampler::try_new(CurveRef::Polyline(&polyline), Tolerance::DEFAULT).unwrap();
    assert_eq!(
        nurbs
            .parameter_at_quadrature_normalized_length(0.5, 0.01)
            .unwrap(),
        1.
    );
    let exact = analytic.parameters_at_normalized_lengths(&[0.5]).unwrap()[0];
    assert_eq!(
        analytic
            .parameter_at_quadrature_normalized_length(0.5, 0.01)
            .unwrap(),
        exact
    );
    assert!((exact - 1.).abs() > 0.009);
}

#[test]
fn numerical_scalar_queries_retain_interior_constant_speed_checkpoints() {
    // Native interval-clock reference: the 0.06 checkpoint of the second
    // span is within 1% of the requested distance, before the affine inverse.
    let curve = NurbsCurve::try_new(
        1,
        [[0.75, 0., 0.], [1., 0., 0.], [1.5, 5e-6, 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0.15, 0.15, 0.2, 0.4, 0.4],
    )
    .unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let numerical = sampler
        .parameter_at_quadrature_normalized_length(0.375, 0.01)
        .unwrap();
    assert!((numerical - 0.212).abs() < 1e-14);
    let precise = sampler.parameters_at_normalized_lengths(&[0.375]).unwrap()[0];
    assert!((precise - 0.21250000000312505).abs() < 1e-14);
    assert!((numerical - precise).abs() > 0.0004);
    assert!(
        (sampler
            .parameter_at_quadrature_normalized_length(0.375, 1e-4)
            .unwrap()
            - precise)
            .abs()
            < 1e-14
    );
}

#[test]
fn large_constant_speed_length_queries_do_not_spend_the_scalar_inverse_step_budget() {
    let segments = 20_000;
    let points = (0..=segments)
        .map(|i| Point3::try_new(i as Real, 0., 0.).unwrap())
        .collect();
    let knots = [
        vec![0.],
        (0..=segments).map(|i| i as Real).collect(),
        vec![segments as Real],
    ]
    .concat();
    let curve = NurbsCurve::try_new(1, points, knots).unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    assert_eq!(sampler.total_length(), segments as Real);
    assert_eq!(
        sampler.quadrature_length_estimate(1e-4).unwrap(),
        segments as Real
    );
}
