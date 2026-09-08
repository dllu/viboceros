use super::*;
use crate::{NurbsCurve, Vector3};

#[test]
fn optional_cache_density_adapts_without_exceeding_aggregate_budget() {
    for (spans, preferred, expected) in [
        (0, 32, None),
        (1, 32, Some(32)),
        (MAX_LOOKUP_NODES / 32, 32, Some(31)),
        (MAX_LOOKUP_NODES / 2, 32, Some(1)),
        (MAX_LOOKUP_NODES / 2 + 1, 32, None),
        (usize::MAX, 32, None),
        (1, usize::MAX, Some(MAX_LOOKUP_NODES - 1)),
    ] {
        let actual = affordable_lookup_subdivisions(spans, preferred).unwrap();
        assert_eq!(actual, expected);
        if let Some(subdivisions) = actual {
            checked_lookup_nodes_per_span(spans, subdivisions).unwrap();
        }
    }
    assert!(affordable_lookup_subdivisions(0, 0).is_err());
    assert!(affordable_lookup_subdivisions(1, 0).is_err());
}

#[test]
fn invalid_lookup_request_preserves_existing_tables() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(0.5, 1., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let curve = curve.try_insert_knot(0.5, 1).unwrap();
    let mut sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    sampler.prepare_repeated_sampling(16).unwrap();
    let original = sampler.lookup_tables.clone();
    let sample = sampler
        .sample_at_distance(sampler.total_length() * 0.37)
        .unwrap();
    for invalid in [0, usize::MAX, MAX_LOOKUP_NODES, MAX_LOOKUP_NODES / 2] {
        assert!(sampler.prepare_repeated_sampling(invalid).is_err());
        assert_eq!(sampler.lookup_tables, original);
        assert_eq!(
            sampler
                .sample_at_distance(sampler.total_length() * 0.37)
                .unwrap(),
            sample
        );
    }
}

#[test]
fn lookup_budget_checks_aggregate_counts_and_overflow_without_allocating() {
    assert_eq!(checked_lookup_nodes_per_span(0, 32).unwrap(), 33);
    assert_eq!(
        checked_lookup_nodes_per_span(1, MAX_LOOKUP_NODES - 1).unwrap(),
        MAX_LOOKUP_NODES
    );
    assert_eq!(
        checked_lookup_nodes_per_span(2, MAX_LOOKUP_NODES / 2 - 1).unwrap(),
        MAX_LOOKUP_NODES / 2
    );
    for (spans, subdivisions) in [
        (1, 0),
        (0, usize::MAX),
        (1, MAX_LOOKUP_NODES),
        (2, MAX_LOOKUP_NODES / 2),
        (usize::MAX, 1),
    ] {
        assert!(matches!(
            checked_lookup_nodes_per_span(spans, subdivisions),
            Err(GeometryError::InvalidArcLengthLookupBudget {
                maximum: MAX_LOOKUP_NODES
            })
        ));
    }
}

#[test]
fn lookup_distance_and_inverse_share_the_same_length_scale() {
    for scale in [1e-150, 1., 1e150] {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(0.5 * scale, scale, 0.).unwrap(),
                Point3::try_new(scale, 0., 0.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let tolerance = Tolerance::try_new(0.01 * scale, 0.01, 1e-10).unwrap();
        let mut sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), tolerance).unwrap();
        sampler.prepare_repeated_sampling(16).unwrap();
        let half_length = sampler.total_length() * 0.5;
        // Symmetry gives an exact station even with loose length tolerance.
        assert!((sampler.parameter_at_distance(half_length).unwrap() - 0.5).abs() < 1e-12);
        assert!(
            (sampler.distance_at_parameter(0.5).unwrap() / scale - half_length / scale).abs()
                < 1e-12
        );
        // Every exact prefix node must use the same public distance unit in
        // both directions, not only the symmetric midpoint.
        let table = &sampler.lookup_tables[0];
        let table_length = table.last().unwrap().length;
        for node in table {
            let expected = (node.length / table_length) * sampler.total_length();
            let actual = sampler.distance_at_parameter(node.parameter).unwrap();
            assert!((actual / scale - expected / scale).abs() < 1e-12);
            assert!(
                (sampler.parameter_at_distance(actual).unwrap() - node.parameter).abs() < 1e-12
            );
        }
        for parameter in [0.013, 0.137, 0.371, 0.499] {
            let left = sampler.distance_at_parameter(parameter).unwrap();
            let right = sampler.distance_at_parameter(1. - parameter).unwrap();
            assert!((left / scale + right / scale - sampler.total_length() / scale).abs() < 1e-12);
        }
    }
}

#[test]
fn normalized_sampler_preserves_multispan_kinks_and_native_parameters() {
    let curve = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(1., 1., 0.).unwrap(),
        ],
        vec![0., 0., 0.5, 1., 1.],
    )
    .unwrap()
    .try_reparameterized(-f64::MAX..=f64::MAX)
    .unwrap();
    let mut sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
    sampler.prepare_repeated_sampling(8).unwrap();
    assert!((sampler.total_length() - 2.).abs() < 1e-12);
    let kinks = sampler.kinks(0.1).unwrap();
    assert_eq!(kinks.len(), 1);
    assert!((kinks[0].distance - 1.).abs() < 1e-12);
    assert_eq!(
        kinks[0].incoming_tangent.as_vector().to_array(),
        [1., 0., 0.]
    );
    assert_eq!(
        kinks[0].outgoing_tangent.as_vector().to_array(),
        [0., 1., 0.]
    );
    let sample = sampler.sample_at_distance(1.).unwrap();
    assert_eq!(sample.parameter(), 0.);
    assert_eq!(sample.point(), Point3::try_new(1., 0., 0.).unwrap());
    for distance in [0., 0.5, 1., 1.5, 2.] {
        let parameter = sampler.parameter_at_distance(distance).unwrap();
        assert!((sampler.distance_at_parameter(parameter).unwrap() - distance).abs() < 1e-12);
    }
    for invalid in [f64::NAN, f64::INFINITY] {
        assert!(sampler.distance_at_parameter(invalid).is_err());
        assert!(sampler.sample_at_distance(invalid).is_err());
    }
}

#[test]
fn composite_linear_samples_preserve_tiny_leaf_intervals() {
    use crate::{CurveSegment3, PolyCurve3, Polyline3};
    let leaf = Polyline3::try_with_parameters(
        [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., f64::from_bits(1), 1.],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let lines = PolyCurve3::try_with_segment_domains(
        leaf.segments().map(CurveSegment3::Line).collect::<Vec<_>>(),
        vec![0., f64::from_bits(1), 1.],
    )
    .unwrap();
    let composite =
        PolyCurve3::try_with_segment_domains(vec![CurveSegment3::Polyline(leaf)], vec![0., 1.])
            .unwrap();
    for curve in [lines, composite] {
        let sampler =
            ArcLengthSampler::try_new(CurveRef::PolyCurve(&curve), Tolerance::DEFAULT).unwrap();
        for distance in [0.25, 0.5, 0.75, 1.25, 1.5, 1.75] {
            let expected = if distance < 1. {
                Point3::try_new(distance, 0., 0.).unwrap()
            } else {
                Point3::try_new(1., distance - 1., 0.).unwrap()
            };
            assert_eq!(sampler.point_at_distance(distance).unwrap(), expected);
            assert_eq!(
                sampler.sample_at_distance(distance).unwrap().point(),
                expected
            );
        }
        assert_eq!(
            sampler
                .sample_at_distance(1.)
                .unwrap()
                .tangent()
                .as_vector()
                .to_array(),
            [0., 1., 0.]
        );
    }
}

#[test]
fn polyline_distance_samples_do_not_round_through_tiny_native_spans() {
    let curve = crate::Polyline3::try_with_parameters(
        [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., f64::from_bits(1), 1.],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let sampler =
        ArcLengthSampler::try_new(CurveRef::Polyline(&curve), Tolerance::DEFAULT).unwrap();
    for distance in [0.25, 0.5, 0.75] {
        let expected = Point3::try_new(distance, 0., 0.).unwrap();
        assert_eq!(sampler.point_at_distance(distance).unwrap(), expected);
        let sample = sampler.sample_at_distance(distance).unwrap();
        assert_eq!(sample.point(), expected);
        assert_eq!(sample.tangent().as_vector().to_array(), [1., 0., 0.]);
        assert_eq!(
            sample.parameter(),
            sampler.parameter_at_distance(distance).unwrap()
        );
    }
    assert_eq!(
        sampler
            .sample_at_distance(1.)
            .unwrap()
            .tangent()
            .as_vector()
            .to_array(),
        [0., 1., 0.]
    );
}

#[test]
fn polyline_division_preserves_points_on_narrow_parameter_domains() {
    use crate::{CurveSegment3, PolyCurve3, Polyline3};
    let vertices = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec();
    for parameters in [
        vec![0., f64::from_bits(1), f64::from_bits(2)],
        vec![
            1.,
            f64::from_bits(1_f64.to_bits() + 1),
            f64::from_bits(1_f64.to_bits() + 2),
        ],
        vec![-1e200, 0., 1e200],
    ] {
        let curve = Polyline3::try_with_parameters(
            vertices.clone(),
            parameters.clone(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let actual = CurveRef::Polyline(&curve)
            .sample_equal_length_points(4, true, Tolerance::DEFAULT)
            .unwrap();
        let expected = [
            [0., 0., 0.],
            [0.5, 0., 0.],
            [1., 0., 0.],
            [1., 0.5, 0.],
            [1., 1., 0.],
        ]
        .map(|p| Point3::try_from(p).unwrap());
        assert_eq!(actual, expected, "{parameters:?}");
        let composite = PolyCurve3::try_with_segment_domains(
            vec![CurveSegment3::Polyline(curve.clone())],
            vec![0., 1.],
        )
        .unwrap();
        assert_eq!(
            CurveRef::PolyCurve(&composite)
                .sample_equal_length_points(4, true, Tolerance::DEFAULT)
                .unwrap(),
            expected
        );
        let sampler =
            ArcLengthSampler::try_new(CurveRef::Polyline(&curve), Tolerance::DEFAULT).unwrap();
        assert_eq!(sampler.parameter_at_distance(1.).unwrap(), parameters[1]);
        assert_eq!(sampler.distance_at_parameter(parameters[1]).unwrap(), 1.);
        assert_eq!(sampler.kinks(0.1).unwrap().len(), 1);
    }
}

#[test]
fn normalized_polycurve_sampling_retains_junction_tangents() {
    use crate::{CurveSegment3, LineSegment, PolyCurve3};
    let points = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]].map(|p| Point3::try_from(p).unwrap());
    let segments = points
        .windows(2)
        .map(|p| CurveSegment3::Line(LineSegment::try_new(p[0], p[1], Tolerance::DEFAULT).unwrap()))
        .collect::<Vec<_>>();
    for parameters in [
        vec![0., f64::from_bits(1), f64::from_bits(2)],
        vec![-1e200, 0., 1e200],
    ] {
        let curve =
            PolyCurve3::try_with_segment_domains(segments.clone(), parameters.clone()).unwrap();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::PolyCurve(&curve), Tolerance::DEFAULT).unwrap();
        let kinks = sampler.kinks(0.1).unwrap();
        assert_eq!(kinks.len(), 1);
        assert_eq!(kinks[0].distance, 1.);
        assert_eq!(
            kinks[0].incoming_tangent.as_vector().to_array(),
            [1., 0., 0.]
        );
        assert_eq!(
            kinks[0].outgoing_tangent.as_vector().to_array(),
            [0., 1., 0.]
        );
        let corner = sampler.sample_at_distance(1.).unwrap();
        assert_eq!(corner.point(), points[1]);
        assert_eq!(corner.parameter(), parameters[1]);
        assert_eq!(sampler.distance_at_parameter(parameters[1]).unwrap(), 1.);
        assert_eq!(
            sampler.point_at_distance(0.5).unwrap(),
            Point3::try_new(0.5, 0., 0.).unwrap()
        );
        assert_eq!(
            sampler.point_at_distance(1.5).unwrap(),
            Point3::try_new(1., 0.5, 0.).unwrap()
        );
    }
}

#[test]
fn polycurve_sampling_preserves_leaf_spans_on_tiny_outer_domains() {
    use crate::{Circle3, CurveSegment3, PolyCurve3};
    let tolerance = Tolerance::DEFAULT;
    let circle = Circle3::try_new(
        Point3::try_new(0., 0., 0.).unwrap(),
        2.,
        UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap();
    let leaf = circle.to_nurbs().unwrap();
    for domain in [
        [0., 1.],
        [1., f64::from_bits(1_f64.to_bits() + 1)],
        [0., f64::from_bits(1)],
        [-f64::MAX / 2., f64::MAX / 2.],
    ] {
        let curve = PolyCurve3::try_with_segment_domains(
            vec![CurveSegment3::NurbsCurve(leaf.clone())],
            domain.to_vec(),
        )
        .unwrap();
        let original = curve.clone();
        let mut sampler = ArcLengthSampler::try_new(CurveRef::PolyCurve(&curve), tolerance)
            .unwrap_or_else(|error| panic!("{domain:?}: {error}"));
        assert!((sampler.total_length() - 4. * std::f64::consts::PI).abs() < 1e-10);
        for cached in [false, true] {
            if cached {
                sampler.prepare_repeated_sampling(16).unwrap();
            }
            for i in 0..=8 {
                let fraction = i as f64 / 8.;
                let distance = sampler.total_length() * fraction;
                let sample = sampler.sample_at_distance(distance).unwrap();
                let expected = circle
                    .point_at_angle(fraction * std::f64::consts::TAU)
                    .unwrap();
                assert!(
                    sample.point().distance_to(expected).unwrap() < 1e-9,
                    "{domain:?}, cached={cached}, i={i}: {:?} != {expected:?}",
                    sample.point()
                );
                assert!(curve.domain().contains(&sample.parameter()));
                assert_eq!(
                    sample.parameter(),
                    sampler.parameter_at_distance(distance).unwrap()
                );
                if domain[0] == 0. && domain[1] == 1. || domain[1] > 1e100 {
                    assert!(
                        (sampler.distance_at_parameter(sample.parameter()).unwrap() - distance)
                            .abs()
                            < 1e-9
                    );
                }
            }
            assert_eq!(sampler.distance_at_parameter(domain[0]).unwrap(), 0.);
            assert_eq!(
                sampler.distance_at_parameter(domain[1]).unwrap(),
                sampler.total_length()
            );
        }
        assert_eq!(curve, original);
    }
}

#[test]
fn polycurve_sampling_conditions_independent_nurbs_leaf_domains() {
    use crate::{CurveSegment3, PolyCurve3};
    let arch = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(0.5, 1., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let tolerance = Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap();
    let reference = CurveRef::NurbsCurve(&arch)
        .sample_equal_length_points(8, true, tolerance)
        .unwrap();
    for domain in [
        1.0..=f64::from_bits(1_f64.to_bits() + 1),
        0.0..=f64::from_bits(1),
        0.0..=1e-200,
        0.0..=1e200,
    ] {
        let curve = PolyCurve3::try_with_segment_domains(
            vec![CurveSegment3::NurbsCurve(
                arch.try_reparameterized(domain.clone()).unwrap(),
            )],
            vec![0., 1.],
        )
        .unwrap();
        let mut sampler = ArcLengthSampler::try_new(CurveRef::PolyCurve(&curve), tolerance)
            .unwrap_or_else(|error| panic!("{domain:?}: {error}"));
        sampler.prepare_repeated_sampling(16).unwrap();
        for (i, expected) in reference.iter().enumerate() {
            let distance = sampler.total_length() * (i as f64 / 8.);
            let sample = sampler.sample_at_distance(distance).unwrap();
            assert!(sample.point().distance_to(*expected).unwrap() < 1e-10);
            assert!(
                (sampler.distance_at_parameter(sample.parameter()).unwrap() - distance).abs()
                    < 1e-10
            );
        }
    }
}

#[test]
fn nurbs_division_points_survive_extreme_parameter_domains() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(0.5, 1., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let t = Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap();
    let reference = CurveRef::NurbsCurve(&curve)
        .sample_equal_length_points(8, true, t)
        .unwrap();
    assert!(
        reference[4]
            .distance_to(Point3::try_new(0.5, 0.5, 0.).unwrap())
            .unwrap()
            < 1e-12
    );
    for domain in [
        1.0..=f64::from_bits(1.0_f64.to_bits() + 1),
        0.0..=f64::from_bits(1),
        -f64::MAX..=f64::MAX,
        0.0..=1e-200,
        0.0..=1e200,
    ] {
        let mapped = curve.try_reparameterized(domain.clone()).unwrap();
        let actual = CurveRef::NurbsCurve(&mapped)
            .sample_equal_length_points(8, true, t)
            .unwrap();
        for (actual, expected) in actual.iter().zip(&reference) {
            assert!(actual.distance_to(*expected).unwrap() < 1e-11, "{domain:?}");
        }
        let mut sampler = ArcLengthSampler::try_new(CurveRef::NurbsCurve(&mapped), t).unwrap();
        sampler.prepare_repeated_sampling(16).unwrap();
        let exact_length = 0.5 * 5_f64.sqrt() + 0.25 * 2_f64.asinh();
        assert!((sampler.total_length() - exact_length).abs() < 1e-11);
        for (i, expected) in reference.iter().enumerate() {
            let distance = sampler.total_length() * (i as Real / 8.0);
            let sample = sampler.sample_at_distance(distance).unwrap();
            assert!(sample.point().distance_to(*expected).unwrap() < 1e-11);
            assert!(domain.contains(&sample.parameter()));
            assert_eq!(
                sample.parameter(),
                sampler.parameter_at_distance(distance).unwrap()
            );
            let tangent = Vector3::try_new(1., 2. - 4. * expected.x(), 0.)
                .unwrap()
                .normalized_nonzero()
                .unwrap();
            assert!(
                sample
                    .tangent()
                    .as_vector()
                    .dot(tangent.as_vector())
                    .unwrap()
                    > 1. - 1e-12
            );
            // One-ulp domains cannot encode interior stations. On
            // well-resolved domains, public parameters must roundtrip.
            if *domain.end() > 1e100 || *domain.end() == 1e-200 {
                let recovered = sampler.distance_at_parameter(sample.parameter()).unwrap();
                assert!((recovered - distance).abs() < 1e-10, "{domain:?}: {i}");
            }
        }
        assert_eq!(sampler.distance_at_parameter(*domain.start()).unwrap(), 0.);
        assert_eq!(
            sampler.distance_at_parameter(*domain.end()).unwrap(),
            sampler.total_length()
        );
    }
}
