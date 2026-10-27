use super::*;

// A unit circle returns to its start halfway through this open source's
// length, then a radius-two semicircle continues with the same tangent.
fn loop_then_arc() -> NurbsCurve {
    let points = [
        [1., 0.],
        [1., 1.],
        [0., 1.],
        [-1., 1.],
        [-1., 0.],
        [-1., -1.],
        [0., -1.],
        [1., -1.],
        [1., 0.],
        [1., 2.],
        [-1., 2.],
        [-3., 2.],
        [-3., 0.],
    ];
    NurbsCurve::try_new_rational(
        2,
        points
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                crate::WeightedPoint3::try_new(
                    Point3::try_new(p[0], p[1], 0.).unwrap(),
                    if i % 2 == 0 { 1. } else { 0.5_f64.sqrt() },
                )
                .unwrap()
            })
            .collect(),
        vec![
            0., 0., 0., 0.125, 0.125, 0.25, 0.25, 0.375, 0.375, 0.5, 0.5, 0.75, 0.75, 1., 1., 1.,
        ],
    )
    .unwrap()
}

#[test]
fn coincident_interval_endpoints_subdivide_without_losing_an_interior_loop() {
    let source = loop_then_arc();
    let before = source.clone();
    assert!(!source.is_closed().unwrap());
    assert_eq!(source.evaluate(0.).unwrap(), source.evaluate(0.5).unwrap());
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT).unwrap();
    let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
    assert_eq!(
        sampler
            .parameters_at_incremental_normalized_lengths(&fractions, 1e-4)
            .unwrap()[4],
        0.5
    );
    assert!(crate::curve_fit::g1::initial_cubic(&source.try_trimmed(0. ..=0.5).unwrap()).is_err());
    assert!(
        !super::super::super::super::super::parameterize::has_tangent_breaks(
            &source,
            1_f64.to_radians()
        )
        .unwrap()
    );
    let tolerance = 0.01;
    let proposal = construct(
        &source,
        tolerance,
        1_f64.to_radians(),
        SplitClock::SourceLookup,
    )
    .unwrap()
    .unwrap();
    assert!(proposal.breaks.contains(&0.25));
    assert!(proposal.breaks.contains(&0.5));
    assert!(proposal.source_parameters.contains(&0.5));
    let mut covered_loop = false;
    for (fractions, parameters) in proposal
        .breaks
        .windows(2)
        .zip(proposal.source_parameters.windows(2))
    {
        assert!(fractions[0] < fractions[1] && parameters[0] < parameters[1]);
        let original = source.try_trimmed(parameters[0]..=parameters[1]).unwrap();
        let fitted = proposal
            .curve
            .try_trimmed(fractions[0]..=fractions[1])
            .unwrap();
        assert!(crate::curve_fit::g1::within_deviation(&original, &fitted, tolerance).unwrap());
        covered_loop |= fitted.control_points().iter().any(|p| p.point().y() < -0.9);
    }
    assert!(covered_loop);
    let joined = crate::try_propose_joined_refit(&source, tolerance, 0.)
        .unwrap()
        .unwrap();
    // The return to the start has two closest parameters. Retain known span
    // anchors so the complete certificate follows the loop in source order.
    let target = joined.knots()[joined.degree()..=joined.control_points().len()]
        .chunk_by(|a, b| a == b)
        .map(|g| g[0])
        .collect::<Vec<_>>();
    assert_eq!(target.len(), proposal.source_parameters.len());
    let pairs = proposal
        .source_parameters
        .iter()
        .zip(target)
        .map(|(&source, target)| [source, target])
        .collect::<Vec<_>>();
    assert!(
        crate::curve_fit::g1::within_deviation_at_pairs(&source, &joined, tolerance, &pairs)
            .unwrap()
    );
    assert!(joined.control_points().iter().any(|p| p.point().y() < -0.9));
    assert_eq!(source, before);
}

#[test]
fn closest_correspondences_keep_native_candidate_parameters_out_of_fractional_maps() {
    let source = loop_then_arc().try_trimmed(0. ..=0.125).unwrap();
    let initial = crate::curve_fit::g1::initial_cubic(&source).unwrap();
    let candidate = crate::curve_fit::g1::adjusted_cubic(&source, &initial, 0.005)
        .unwrap()
        .unwrap();
    // These fits need closest-station correspondence rather than the affine
    // or coordinate-projection shortcut. Their stored domains stay intact.
    for (start, end) in [
        (0., 1.),
        (-3., 17.),
        (100., 107.),
        (-1e100, 1e100),
        (1e-100, 2e-100),
    ] {
        let target = candidate.try_reparameterized(start..=end).unwrap();
        let before = (source.clone(), target.clone());
        assert!(
            source
                .parameter_curve_deviation_bound(&target, 0.01)
                .unwrap()
                .is_none()
        );
        assert!(
            source
                .monotone_projected_curve_deviation_bound(&target, 0.01)
                .unwrap()
                .is_none()
        );
        assert!(
            crate::curve_fit::g1::within_deviation(&source, &target, 0.01).unwrap(),
            "{start}..{end}"
        );
        assert!(!crate::curve_fit::g1::within_deviation(&source, &target, 1e-6).unwrap());
        assert_eq!((source.clone(), target), before);
    }
}

#[test]
fn a_coincident_interval_keeps_the_depth_and_representable_split_limits() {
    let interval = Interval {
        start: 0.,
        end: 1.,
        from: 0.,
        to: 1.,
        depth: MAX_DEPTH,
    };
    assert!(interval.subdivide(0.5, &[]).is_none());
    let interval = Interval {
        depth: 0,
        ..interval
    };
    for middle in [0., 1.] {
        assert!(interval.subdivide(middle, &[]).is_none());
    }
    let children = interval.subdivide(0.5_f64.next_up(), &[0.5]).unwrap();
    assert_eq!((children[0].start, children[1].end), (0.5, 0.5));
    assert_eq!((children[0].depth, children[1].depth), (1, 1));
}
