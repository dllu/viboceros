use super::*;
use crate::{NurbsCurve, WeightedPoint3};

#[test]
fn rational_line_batch_inverts_physical_lengths_in_a_nonunit_domain() {
    let point = |x| Point3::try_new(x, 0., 0.).unwrap();
    let curve = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(point(0.), 3.).unwrap(),
            WeightedPoint3::try_new(point(10.), 7.).unwrap(),
        ],
        vec![10., 10., 20., 20.],
    )
    .unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let fractions = [1., 0.5, 0., 0.75, 0.5, -0., 0.25];
    let parameters = sampler
        .parameters_at_normalized_lengths(&fractions)
        .unwrap();
    for (f, t) in fractions.into_iter().zip(parameters) {
        let expected = 10. + 10. * (3. * f) / (7. - 4. * f);
        assert!((t - expected).abs() < 4e-15, "{f}: {t} != {expected}");
        assert!((curve.evaluate(t).unwrap().x() - 10. * f).abs() < 1e-14);
    }
    assert!(
        sampler
            .parameters_at_normalized_lengths(&[])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn batch_validation_is_bounded_and_does_not_change_prepared_geometry() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 2., 1.).unwrap(),
            Point3::try_new(4., 0., 3.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let mut sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    sampler.prepare_budgeted_repeated_sampling(32).unwrap();
    let before = sampler.lookup_tables.clone();
    let result = sampler
        .parameters_at_normalized_lengths(&[0., 0.25, 0.5, 1.])
        .unwrap();
    for bad in [-0.1, 1.1, Real::NAN, Real::INFINITY, Real::NEG_INFINITY] {
        assert!(
            sampler
                .parameters_at_normalized_lengths(&[0., 0.5, bad])
                .is_err()
        );
    }
    assert!(matches!(
        sampler.parameters_at_normalized_lengths(&vec![0.; MAX_LOOKUP_NODES + 1]),
        Err(GeometryError::ArcLengthBatchResourceLimit { .. })
    ));
    assert_eq!(sampler.lookup_tables, before);
    assert_eq!(
        sampler
            .parameters_at_normalized_lengths(&[0., 0.25, 0.5, 1.])
            .unwrap(),
        result
    );
}

#[test]
fn batch_retains_exact_endpoints_for_adjacent_and_unshiftable_native_domains() {
    for (start, end) in [
        (1e100, f64::from_bits(1e100_f64.to_bits() + 1)),
        (1e100, f64::from_bits(1e100_f64.to_bits() + 2)),
    ] {
        let curve = NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(2., 0., 0.).unwrap(),
            ],
            vec![start, start, end, end],
        )
        .unwrap();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        let p = sampler
            .parameters_at_normalized_lengths(&[1., 0., 0.5, 1., 0.])
            .unwrap();
        assert_eq!(p[0], end);
        assert_eq!(p[1], start);
        assert_eq!(p[3], end);
        assert_eq!(p[4], start);
        assert!((start..=end).contains(&p[2]));
    }
}
