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
fn sampling_cannot_hide_a_bump_between_the_interior_stations() {
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
    let fit = try_propose_adaptive_linear_refit(&source, 1e-7)
        .unwrap()
        .unwrap();
    assert!(fit.curve.control_points().len() > 4);
    assert!(fit.maximum_deviation_bound <= 1e-7);
    let peak = Point3::try_new(0.05, 1e-6, 0.).unwrap();
    let search = fit.curve.prepare_closest_parameter_search().unwrap();
    let parameter = search.closest_parameter(peak, Tolerance::DEFAULT).unwrap();
    assert!(
        peak.distance_to(fit.curve.evaluate(parameter).unwrap())
            .unwrap()
            <= 1e-7
    );
    assert!(crate::curve_fit::g1::within_deviation(&source, &fit.curve, 1e-7).unwrap());
    assert_eq!(source, before);
}

#[test]
fn separate_small_features_are_retained_across_reversal_and_model_units() {
    for scale in [1., 1024.] {
        let points = [
            [0., 0.],
            [0.049, 0.],
            [0.05, 1e-6],
            [0.051, 0.],
            [0.949, 0.],
            [0.95, -1e-6],
            [0.951, 0.],
            [1., 0.],
        ]
        .map(|p| p.map(|c| c * scale));
        let forward = polyline(&points);
        for source in [forward.clone(), forward.reversed().unwrap()] {
            let before = source.clone();
            let limit = 1e-7 * scale;
            let fit = try_propose_adaptive_linear_refit(&source, limit)
                .unwrap()
                .unwrap();
            assert!(fit.maximum_deviation_bound <= limit);
            let joined = try_propose_joined_linear_refit(&source, limit, 0.)
                .unwrap()
                .unwrap();
            for curve in [&fit.curve, &joined] {
                let search = curve.prepare_closest_parameter_search().unwrap();
                let numerical = Tolerance::try_new(limit * 1e-4, 1e-12, 1e-10).unwrap();
                for p in [points[2], points[5]] {
                    let peak = Point3::try_new(p[0], p[1], 0.).unwrap();
                    let t = search.closest_parameter(peak, numerical).unwrap();
                    assert!(peak.distance_to(curve.evaluate(t).unwrap()).unwrap() <= limit);
                }
                assert!(crate::curve_fit::g1::within_deviation(&source, curve, limit).unwrap());
            }
            assert_eq!(source, before);
        }
    }
}

#[test]
fn unresolved_features_exhaust_the_existing_piece_budget_without_partial_output() {
    let mut points = vec![[0., 0.]];
    for index in 1..=168 {
        let center = index as Real * 0.0002;
        points.extend([[center - 1e-5, 0.], [center, 2e-8], [center + 1e-5, 0.]]);
    }
    points.push([1., 0.]);
    let source = polyline(&points);
    let before = source.clone();
    assert!(source::linear_image_proxy(&source).unwrap().is_some());
    assert!(!crate::loft::parameterize::has_tangent_breaks(&source, 1_f64.to_radians()).unwrap());
    assert!(
        try_propose_adaptive_linear_refit(&source, 1e-9)
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

#[test]
fn joined_proposals_preserve_the_source_and_require_a_complete_final_bound() {
    let angle = 0.25_f64.to_radians();
    for scale in [1., 2., 4., 1000.] {
        let source = polyline(&[
            [0., 0.],
            [scale, 0.],
            [scale * (1. + angle.cos()), scale * angle.sin()],
        ]);
        let before = source.clone();
        let limit = 0.001 * scale;
        let joined = try_propose_joined_linear_refit(&source, limit, 0.)
            .unwrap()
            .unwrap();
        assert!(
            source
                .parameter_curve_deviation_bound(&joined, limit)
                .unwrap()
                .is_some()
        );
        assert_eq!(source, before);
        // A certified proposal does not establish the native subdivision rule.
        assert!(try_prepare_refitted_loft_sections(&[source.clone(), source], limit).is_err());
    }
}

#[test]
fn joined_proposals_validate_angles_before_constructing_any_output() {
    let source = polyline(&[[0., 0.], [1., 0.], [2., 1e-5]]);
    for angle in [-1., Real::NAN, Real::INFINITY, 2. * std::f64::consts::PI] {
        assert!(matches!(
            try_propose_joined_linear_refit(&source, 1e-6, angle),
            Err(GeometryError::InvalidContinuityAngle)
        ));
    }
}
