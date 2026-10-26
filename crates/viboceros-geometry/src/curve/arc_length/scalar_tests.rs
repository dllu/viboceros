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
