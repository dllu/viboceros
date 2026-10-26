use super::*;
use crate::{NurbsCurve, WeightedPoint3};

fn circle(scale: Real, domain: [Real; 2]) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [[1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
            .into_iter()
            .zip([1., 0.5_f64.sqrt(), 1.])
            .map(|(p, w)| {
                WeightedPoint3::try_new(Point3::try_from(p.map(|x| x * scale)).unwrap(), w).unwrap()
            })
            .collect(),
        vec![
            domain[0], domain[0], domain[0], domain[1], domain[1], domain[1],
        ],
    )
    .unwrap()
}

#[test]
fn incremental_queries_depend_on_the_prefix_and_preserve_its_results() {
    let curve = circle(1., [0., 1.]);
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let fractions = (0..=840).map(|i| i as Real / 840.).collect::<Vec<_>>();
    let all = sampler
        .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
        .unwrap();
    let prefix = sampler
        .parameters_at_incremental_normalized_lengths(&fractions[..421], 1e-6)
        .unwrap();
    let isolated = sampler
        .parameters_at_incremental_normalized_lengths(&[0.5], 1e-6)
        .unwrap();
    assert_eq!(&all[..421], prefix);
    assert!((all[420] - isolated[0]).abs() > 1e-5);
    assert_eq!(all[0], 0.);
    assert_eq!(all[840], 1.);
    assert!(all.windows(2).all(|p| p[0] <= p[1]));
    let repeated = sampler
        .parameters_at_incremental_normalized_lengths(&[0., 0., 0.5, 0.5, 1., 1.], 1e-6)
        .unwrap();
    assert_eq!(repeated[0], repeated[1]);
    assert_eq!(repeated[2], repeated[3]);
    assert_eq!(repeated[4], repeated[5]);
}

#[test]
fn scales_and_source_domains_retain_the_same_incremental_inverse() {
    let fractions = [0., 0.125, 0.25, 0.5, 0.75, 0.875, 1.];
    let reference = circle(1., [0., 1.]);
    let reference = ArcLengthSampler::try_new(CurveRef::NurbsCurve(&reference), Tolerance::DEFAULT)
        .unwrap()
        .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
        .unwrap();
    for scale in [1e-200, 1., 1e200] {
        let curve = circle(scale, [10., 40.]);
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        let actual = sampler
            .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
            .unwrap();
        for (a, b) in actual.iter().zip(&reference) {
            assert!(((a - 10.) / 30. - b).abs() < 1e-8);
        }
    }
    let start = 1e12_f64;
    let curve = circle(1., [start, start.next_up()]);
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let actual = sampler
        .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
        .unwrap();
    assert_eq!(actual[0], start);
    assert_eq!(actual[6], start.next_up());
    assert!(actual.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn invalid_batches_fail_without_changing_the_precise_sampler() {
    let curve = circle(1., [0., 1.]);
    let mut sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    sampler.prepare_budgeted_repeated_sampling(64).unwrap();
    let before = sampler
        .parameters_at_normalized_lengths(&[0., 0.2, 0.5, 1.])
        .unwrap();
    for fractions in [
        &[-0.1][..],
        &[1.1],
        &[Real::NAN],
        &[Real::INFINITY],
        &[0.5, 0.4],
    ] {
        assert!(
            sampler
                .parameters_at_incremental_normalized_lengths(fractions, 1e-6)
                .is_err()
        );
    }
    for tolerance in [0., 1., -1., Real::NAN, Real::INFINITY] {
        assert_eq!(
            sampler
                .parameters_at_incremental_normalized_lengths(&[0.5], tolerance)
                .err(),
            Some(GeometryError::InvalidArcLengthIntegrationTolerance)
        );
    }
    assert!(
        sampler
            .parameters_at_incremental_normalized_lengths(&[], 1e-6)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        sampler.parameters_at_incremental_normalized_lengths(&vec![0.; MAX_LOOKUP_NODES + 1], 1e-6),
        Err(GeometryError::ArcLengthBatchResourceLimit { .. })
    ));
    assert!(matches!(
        sampler.incremental_prefix(
            Node {
                parameter: 0.,
                distance: 0.
            },
            0.5,
            1e-6,
            1.,
            &mut 0
        ),
        Err(GeometryError::NumericalIntegrationDidNotConverge)
    ));
    assert_eq!(
        before,
        sampler
            .parameters_at_normalized_lengths(&[0., 0.2, 0.5, 1.])
            .unwrap()
    );
}

#[test]
fn span_boundaries_use_local_derivatives_and_retain_physical_lengths() {
    let curve = NurbsCurve::try_new(
        2,
        [
            [0., 0., 0.],
            [0.5, 0., 0.],
            [1., 0., 0.],
            [1., 2., 0.],
            [1., 4., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    let fractions = [0., 0.1, 0.2, 0.3, 0.5, 0.75, 1.];
    let actual = sampler
        .parameters_at_incremental_normalized_lengths(&fractions, 1e-8)
        .unwrap();
    for (f, t) in fractions.into_iter().zip(actual) {
        let expected = if f <= 0.2 {
            2.5 * f
        } else {
            0.5 + 0.625 * (f - 0.2)
        };
        assert!((t - expected).abs() < 1e-12);
    }
}

#[test]
fn composite_inverse_scales_each_component_error_by_its_own_length() {
    let arc = circle(1., [0., 1.]);
    let line = crate::LineSegment::try_new(
        Point3::try_new(0., 1., 0.).unwrap(),
        Point3::try_new(-1., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let poly = crate::PolyCurve3::try_new(vec![
        crate::CurveSegment3::NurbsCurve(arc.clone()),
        crate::CurveSegment3::Line(line),
    ])
    .unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::PolyCurve(&poly), Tolerance::DEFAULT).unwrap();
    let single = ArcLengthSampler::try_new(CurveRef::NurbsCurve(&arc), Tolerance::DEFAULT).unwrap();
    let length = single
        .integration_checkpoints(1e-6)
        .unwrap()
        .last()
        .unwrap()
        .distance();
    let fractions = [0., 0.1, 0.2, 0.3, 0.4, 0.5];
    let local = fractions.map(|f| f * (length + 1.) / length);
    let expected = single
        .parameters_at_incremental_normalized_lengths(&local, 1e-6)
        .unwrap();
    let actual = sampler
        .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
        .unwrap();
    for (a, b) in actual.iter().zip(expected) {
        assert!((a - b).abs() < 1e-9);
    }
}
