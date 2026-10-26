use super::*;

fn polyline(points: &[[Real; 2]]) -> NurbsCurve {
    NurbsCurve::try_new(
        1,
        points
            .iter()
            .map(|p| Point3::try_new(p[0], p[1], 0.).unwrap())
            .collect(),
        [
            vec![0.],
            (0..points.len()).map(|i| i as Real).collect(),
            vec![(points.len() - 1) as Real],
        ]
        .concat(),
    )
    .unwrap()
}

#[test]
fn complete_bounds_cover_every_adaptive_piece_and_preserve_the_source() {
    let source = polyline(&[[0., 0.], [1., 0.], [2., 1e-5]]);
    let before = source.clone();
    let fit = try_propose_adaptive_linear_refit(&source, 1e-6)
        .unwrap()
        .unwrap();
    assert_eq!(fit.arc_length_breaks, [0., 0.25, 0.375, 0.5, 1.]);
    assert_eq!(fit.curve.control_points().len(), 13);
    assert!(fit.maximum_deviation_bound <= 1e-6);
    assert!(
        source
            .parameter_curve_deviation_bound(&fit.curve, 1e-6)
            .unwrap()
            .is_some()
    );
    assert_eq!(source, before);
}

#[test]
fn the_sample_reserve_is_separate_from_the_complete_requested_tolerance() {
    let angle = 0.05_f64.to_radians();
    let source = polyline(&[[0., 0.], [1., 0.], [1. + angle.cos(), angle.sin()]]);
    let fit = try_propose_adaptive_linear_refit(&source, 1e-6)
        .unwrap()
        .unwrap();
    let end = fit
        .arc_length_breaks
        .iter()
        .position(|t| (*t - 0.5).abs() < 1e-14)
        .unwrap();
    let [a, b] = [fit.arc_length_breaks[end - 1], fit.arc_length_breaks[end]];
    let point = fit.curve.evaluate(a + (b - a) * 2. / 3.).unwrap();
    // The source's first segment is horizontal. This interior point lies
    // between its endpoints, and farther than the reserve from the source.
    assert!(point.x() > 0. && point.x() < 1.);
    assert!(point.y() < -0.5e-6);
    assert!(fit.maximum_deviation_bound <= 1e-6);
}

#[test]
fn sampling_cannot_hide_a_bump_between_all_seven_interior_stations() {
    let source = polyline(&[[0., 0.], [0.049, 0.], [0.05, 1e-6], [0.051, 0.], [1., 0.]]);
    let sampler =
        ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT).unwrap();
    let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
    let parameters = sampler
        .parameters_at_incremental_normalized_lengths(&fractions, 1e-4)
        .unwrap();
    for t in parameters {
        assert_eq!(source.evaluate(t).unwrap().y(), 0.);
    }
    let before = source.clone();
    assert!(
        try_propose_adaptive_linear_refit(&source, 1e-7)
            .unwrap()
            .is_none()
    );
    assert_eq!(source, before);
}

#[test]
fn invalid_tolerances_resolved_corners_and_depth_exhaustion_return_no_output() {
    let source = polyline(&[[0., 0.], [1., 0.], [2., 1e-5]]);
    for limit in [0., -1., Real::NAN, Real::INFINITY] {
        assert!(matches!(
            try_propose_adaptive_linear_refit(&source, limit),
            Err(GeometryError::InvalidTolerance)
        ));
    }
    assert!(
        try_propose_adaptive_linear_refit(&source, 1e-25)
            .unwrap()
            .is_none()
    );
    let corner = polyline(&[[0., 0.], [1., 0.], [1., 1.]]);
    assert!(
        try_propose_adaptive_linear_refit(&corner, 0.01)
            .unwrap()
            .is_none()
    );
}
